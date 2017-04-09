use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use disk::fs::FileSystem;
use disk::{IDiskMessage, ODiskMessage};
use disk::tasks;

use futures::sync::mpsc::{self, Sender, Receiver};
use futures::{StartSend, Poll, Stream, Sink, AsyncSink, Async};
use futures_cpupool::{Builder, CpuPool};

/// `DiskManager` object which handles the storage of `Blocks` to the `FileSystem`.
pub struct DiskManager<F> where F: FileSystem {
    sink:   DiskManagerSink<F>,
    stream: DiskManagerStream
}

pub fn new_manager<F>(pending_size: usize, completed_size: usize, fs: F, mut builder: Builder) -> DiskManager<F>
    where F: FileSystem {
    let (out_send, out_recv) = mpsc::channel(completed_size);

    DiskManager{ sink: DiskManagerSink::new(builder.create(), out_send, fs, pending_size),
                 stream: DiskManagerStream::new(out_recv) }
}

impl<F> Sink for DiskManager<F> where F: FileSystem {
    type SinkItem = IDiskMessage;
    type SinkError = ();

    fn start_send(&mut self, item: IDiskMessage) -> StartSend<IDiskMessage, ()> {
        self.sink.start_send(item)
    }
    
    fn poll_complete(&mut self) -> Poll<(), ()> {
        self.sink.poll_complete()
    }
}

impl<F> Stream for DiskManager<F> where F: FileSystem {
    type Item = ODiskMessage;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<ODiskMessage>, ()> {
        self.stream.poll()
    }
}

//----------------------------------------------------------------------------//

pub struct DiskManagerSink<F> where F: FileSystem {
    cpu_pool:    CpuPool,
    out_send:    Sender<ODiskMessage>,
    filesystem:  Arc<F>,
    cur_pending: Arc<AtomicUsize>,
    max_pending: usize
}

impl<F> DiskManagerSink<F> where F: FileSystem {
    fn new(cpu_pool: CpuPool, out_send: Sender<ODiskMessage>, filesystem: F, max_pending: usize) -> DiskManagerSink<F> {
        DiskManagerSink{ cpu_pool: cpu_pool, out_send: out_send, filesystem: Arc::new(filesystem),
                         cur_pending: Arc::new(AtomicUsize::new(0)), max_pending: max_pending}
    }
}

impl<F> Sink for DiskManagerSink<F> where F: FileSystem {
    type SinkItem = IDiskMessage;
    type SinkError = ();

    fn start_send(&mut self, item: IDiskMessage) -> StartSend<IDiskMessage, ()> {
        let new_value = self.cur_pending.fetch_add(1, Ordering::SeqCst);

        if new_value <= self.max_pending {
            tasks::execute_on_pool(&self.cpu_pool, self.cur_pending.clone(), self.out_send.clone(),
                                  self.filesystem.clone(), item);

            Ok(AsyncSink::Ready)
        } else {
            self.cur_pending.fetch_sub(1, Ordering::SeqCst);

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
        self.recv.poll()
    }
}