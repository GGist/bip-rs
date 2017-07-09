use disk::fs::FileSystem;
use disk::{IDiskMessage, ODiskMessage};
use disk::tasks;
use disk::tasks::context::DiskManagerContext;
use disk::builder::DiskManagerBuilder;

use futures::task::{self, Task};
use futures::sync::mpsc::{self, Receiver};
use futures::{StartSend, Poll, Stream, Sink, AsyncSink, Async};
use futures_cpupool::{CpuPool};

/// `DiskManager` object which handles the storage of `Blocks` to the `FileSystem`.
pub struct DiskManager<F> {
    pool:     CpuPool,
    context:  DiskManagerContext<F>,
    recv:     Receiver<ODiskMessage>,
    opt_task: Option<Task>
}

impl<F> DiskManager<F> {
    /// Create a `DiskManager` from the given `DiskManagerBuilder`.
    pub fn from_builder(mut builder: DiskManagerBuilder, fs: F) -> DiskManager<F> {
        let sink_capacity = builder.sink_buffer_capacity();
        let stream_capacity = builder.stream_buffer_capacity();
        let pool_builder = builder.worker_config();

        let (out_send, out_recv) = mpsc::channel(stream_capacity);
        let context = DiskManagerContext::new(out_send, fs, sink_capacity);

        DiskManager{ pool: pool_builder.create(), context: context, recv: out_recv, opt_task: None }
    }
}

impl<F> Sink for DiskManager<F> where F: FileSystem + Send + Sync + 'static {
    type SinkItem = IDiskMessage;
    type SinkError = ();

    fn start_send(&mut self, item: IDiskMessage) -> StartSend<IDiskMessage, ()> {
        info!("Starting Send For DiskManagerSink With IDiskMessage");

        if self.context.try_submit_work() {
            info!("DiskManagerSink Ready For New Work");

            tasks::execute_on_pool(item, &self.pool, self.context.clone());

            info!("DiskManagerSink Submitted Work To Pool");

            Ok(AsyncSink::Ready)
        } else {
            info!("DiskManagerSink Not Ready For New Work");
            self.opt_task = Some(task::current());

            Ok(AsyncSink::NotReady(item))
        }
    }
    
    fn poll_complete(&mut self) -> Poll<(), ()> {
        Ok(Async::Ready(()))
    }
}

impl<F> Stream for DiskManager<F> {
    type Item = ODiskMessage;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<ODiskMessage>, ()> {
        info!("Polling DiskManagerStream For ODiskMessage");
        info!("Notifying DiskManager That We Can Submit More Work");

        // TODO: Should only do this for certain message types
        self.opt_task.take().map(|task| task.notify());
        
        self.recv.poll()
    }
}