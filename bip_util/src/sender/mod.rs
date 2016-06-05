use std::sync::mpsc;

mod priority_sender;

pub use sender::priority_sender::{PrioritySender, PrioritySenderAck};

/// Trait for generic sender implementations.
pub trait Sender<T: Send>: Send {
    /// Send data through the concrete channel.
    fn send(&self, data: T);
}

impl<T: Send> Sender<T> for mpsc::Sender<T> {
    #[allow(unused)]
    fn send(&self, data: T) {
        mpsc::Sender::send(self, data);
    }
}

impl<T: Send> Sender<T> for mpsc::SyncSender<T> {
    #[allow(unused)]
    fn send(&self, data: T) {
        mpsc::SyncSender::try_send(self, data);
    }
}
