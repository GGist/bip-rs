#![allow(unused)]

use std::sync::mpsc::{self};

use mio::{self};

/// Trait for handshakers to send data back to the client.
pub trait Channel<T: Send>: Send {
    /// Send data back to the client.
    ///
    /// Consumers will expect this method to not block.
    fn send(&mut self, data: T);
}

impl<T: Send> Channel<T> for mpsc::Sender<T> {
    fn send(&mut self, data: T) {
        mpsc::Sender::send(self, data);
    }
}

impl<T: Send> Channel<T> for mpsc::SyncSender<T> {
    fn send(&mut self, data: T) {
        mpsc::SyncSender::try_send(self, data);
    }
}

impl<T: Send> Channel<T> for mio::Sender<T> {
    fn send(&mut self, data: T) {
        mio::Sender::send(self, data);
    }
}