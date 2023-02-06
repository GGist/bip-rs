use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::disk::builder::DiskManagerBuilder;
use crate::disk::fs::FileSystem;
use crate::disk::tasks;
use crate::disk::tasks::context::DiskManagerContext;
use crate::disk::{IDiskMessage, ODiskMessage};

use crossbeam::queue::SegQueue;
use futures::sync::mpsc::{self, Receiver};
use futures::task::{self, Task};
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use futures_cpupool::CpuPool;

/// `DiskManager` object which handles the storage of `Blocks` to the
/// `FileSystem`.
pub struct DiskManager<F> {
    sink: DiskManagerSink<F>,
    stream: DiskManagerStream,
}

impl<F> DiskManager<F> {
    /// Create a `DiskManager` from the given `DiskManagerBuilder`.
    pub fn from_builder(mut builder: DiskManagerBuilder, fs: F) -> DiskManager<F> {
        let cur_sink_capacity = Arc::new(AtomicUsize::new(0));
        let sink_capacity = builder.sink_buffer_capacity();
        let stream_capacity = builder.stream_buffer_capacity();
        let pool_builder = builder.worker_config();

        let (out_send, out_recv) = mpsc::channel(stream_capacity);
        let context = DiskManagerContext::new(out_send, fs);
        let task_queue = Arc::new(SegQueue::new());

        let sink = DiskManagerSink::new(
            pool_builder.create(),
            context,
            sink_capacity,
            cur_sink_capacity.clone(),
            task_queue.clone(),
        );
        let stream = DiskManagerStream::new(out_recv, cur_sink_capacity, task_queue);

        DiskManager { sink, stream }
    }

    /// Break the `DiskManager` into a sink and stream.
    ///
    /// The returned sink implements `Clone`.
    pub fn into_parts(self) -> (DiskManagerSink<F>, DiskManagerStream) {
        (self.sink, self.stream)
    }
}

impl<F> Sink for DiskManager<F>
where
    F: FileSystem + Send + Sync + 'static,
{
    type SinkItem = IDiskMessage;
    type SinkError = ();

    fn start_send(&mut self, item: IDiskMessage) -> StartSend<IDiskMessage, ()> {
        self.sink.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), ()> {
        self.sink.poll_complete()
    }
}

impl<F> Stream for DiskManager<F> {
    type Item = ODiskMessage;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<ODiskMessage>, ()> {
        self.stream.poll()
    }
}

//----------------------------------------------------------------------------//

/// `DiskManagerSink` which is the sink portion of a `DiskManager`.
pub struct DiskManagerSink<F> {
    pool: CpuPool,
    context: DiskManagerContext<F>,
    max_capacity: usize,
    cur_capacity: Arc<AtomicUsize>,
    task_queue: Arc<SegQueue<Task>>,
}

impl<F> Clone for DiskManagerSink<F> {
    fn clone(&self) -> DiskManagerSink<F> {
        DiskManagerSink {
            pool: self.pool.clone(),
            context: self.context.clone(),
            max_capacity: self.max_capacity,
            cur_capacity: self.cur_capacity.clone(),
            task_queue: self.task_queue.clone(),
        }
    }
}

impl<F> DiskManagerSink<F> {
    fn new(
        pool: CpuPool,
        context: DiskManagerContext<F>,
        max_capacity: usize,
        cur_capacity: Arc<AtomicUsize>,
        task_queue: Arc<SegQueue<Task>>,
    ) -> DiskManagerSink<F> {
        DiskManagerSink {
            pool,
            context,
            max_capacity,
            cur_capacity,
            task_queue,
        }
    }

    fn try_submit_work(&self) -> bool {
        let cur_capacity = self.cur_capacity.fetch_add(1, Ordering::SeqCst);

        if cur_capacity < self.max_capacity {
            true
        } else {
            self.cur_capacity.fetch_sub(1, Ordering::SeqCst);

            false
        }
    }
}

impl<F> Sink for DiskManagerSink<F>
where
    F: FileSystem + Send + Sync + 'static,
{
    type SinkItem = IDiskMessage;
    type SinkError = ();

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        info!("Starting Send For DiskManagerSink With IDiskMessage");

        if self.try_submit_work() {
            info!("DiskManagerSink Submitted Work On First Attempt");
            tasks::execute_on_pool(item, &self.pool, self.context.clone());

            return Ok(AsyncSink::Ready);
        }

        // We split the sink and stream, which means these could be polled in different
        // event loops (I think), so we need to add our task, but then try to
        // sumbit work again, in case the receiver processed work right after we
        // tried to submit the first time.
        info!("DiskManagerSink Failed To Submit Work On First Attempt, Adding Task To Queue");
        self.task_queue.push(task::current());

        if self.try_submit_work() {
            // Receiver will look at the queue but wake us up, even though we dont need it
            // to now...
            info!("DiskManagerSink Submitted Work On Second Attempt");
            tasks::execute_on_pool(item, &self.pool, self.context.clone());

            Ok(AsyncSink::Ready)
        } else {
            // Receiver will look at the queue eventually...
            Ok(AsyncSink::NotReady(item))
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

//----------------------------------------------------------------------------//

/// `DiskManagerStream` which is the stream portion of a `DiskManager`.
pub struct DiskManagerStream {
    recv: Receiver<ODiskMessage>,
    cur_capacity: Arc<AtomicUsize>,
    task_queue: Arc<SegQueue<Task>>,
}

impl DiskManagerStream {
    fn new(
        recv: Receiver<ODiskMessage>,
        cur_capacity: Arc<AtomicUsize>,
        task_queue: Arc<SegQueue<Task>>,
    ) -> DiskManagerStream {
        DiskManagerStream {
            recv,
            cur_capacity,
            task_queue,
        }
    }

    fn complete_work(&self) {
        self.cur_capacity.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Stream for DiskManagerStream {
    type Item = ODiskMessage;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<ODiskMessage>, ()> {
        info!("Polling DiskManagerStream For ODiskMessage");

        match self.recv.poll() {
            res @ Ok(Async::Ready(Some(ODiskMessage::TorrentAdded(_))))
            | res @ Ok(Async::Ready(Some(ODiskMessage::TorrentRemoved(_))))
            | res @ Ok(Async::Ready(Some(ODiskMessage::TorrentSynced(_))))
            | res @ Ok(Async::Ready(Some(ODiskMessage::BlockLoaded(_))))
            | res @ Ok(Async::Ready(Some(ODiskMessage::BlockProcessed(_)))) => {
                self.complete_work();

                info!("Notifying DiskManager That We Can Submit More Work");
                loop {
                    match self.task_queue.pop() {
                        Some(task) => task.notify(),
                        None => {
                            break;
                        }
                    }
                }

                res
            }
            other => other,
        }
    }
}
