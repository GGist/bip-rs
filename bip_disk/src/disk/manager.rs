use disk::fs::FileSystem;
use disk::{IDiskMessage, ODiskMessage};
use disk::tasks;
use disk::tasks::context::DiskManagerContext;

use futures::sync::mpsc::{self, Receiver};
use futures::{StartSend, Poll, Stream, Sink, AsyncSink, Async};
use futures_cpupool::{Builder, CpuPool};

/// `DiskManager` object which handles the storage of `Blocks` to the `FileSystem`.
pub struct DiskManager<F> {
    sink:   DiskManagerSink<F>,
    stream: DiskManagerStream
}

pub fn new_manager<F>(pending_size: usize, completed_size: usize, fs: F, mut builder: Builder) -> DiskManager<F>
    where F: FileSystem {
    let (out_send, out_recv) = mpsc::channel(completed_size);
    let context = DiskManagerContext::new(out_send, fs, pending_size);

    DiskManager{ sink: DiskManagerSink::new(builder.create(), context), stream: DiskManagerStream::new(out_recv) }
}

impl<F> Sink for DiskManager<F> where F: FileSystem + Send + Sync + 'static {
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

pub struct DiskManagerSink<F> {
    pool:    CpuPool,
    context: DiskManagerContext<F>
}

impl<F> DiskManagerSink<F> {
    fn new(pool: CpuPool, context: DiskManagerContext<F>) -> DiskManagerSink<F> {
        DiskManagerSink{ pool: pool, context: context }
    }
}

impl<F> Sink for DiskManagerSink<F> where F: FileSystem + Send + Sync + 'static {
    type SinkItem = IDiskMessage;
    type SinkError = ();

    fn start_send(&mut self, item: IDiskMessage) -> StartSend<IDiskMessage, ()> {
        info!("Starting Send For DiskManagerSink With IDiskMessage");

        if self.context.can_submit_work() {
            info!("DiskManagerSink Ready For New Work");

            tasks::execute_on_pool(item, &self.pool, self.context.clone());

            info!("DiskManagerSink Submitted Work To Pool");

            Ok(AsyncSink::Ready)
        } else {
            info!("DiskManagerSink Not Ready For New Work");

            Ok(AsyncSink::NotReady(item))
        }
    }
    
    fn poll_complete(&mut self) -> Poll<(), ()> {
        Ok(Async::Ready(()))
    }
}

//----------------------------------------------------------------------------//

pub struct DiskManagerStream {
    recv: Receiver<ODiskMessage>
}

impl DiskManagerStream {
    fn new(recv: Receiver<ODiskMessage>) -> DiskManagerStream {
        DiskManagerStream{ recv: recv }
    }
}

impl Stream for DiskManagerStream {
    type Item = ODiskMessage;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<ODiskMessage>, ()> {
        info!("Polling DiskManagerStream For ODiskMessage");

        self.recv.poll()
    }
}