use std::sync::mpsc::{self, TrySendError};

mod split_sender;

pub use crate::send::split_sender::{SplitSender, SplitSenderAck, split_sender};

/// Trait for generic sender implementations.
pub trait TrySender<T: Send>: Send {
    /// Send data through the concrete channel.
    ///
    /// If the channel is full, return the data back to the caller; if
    /// the channel has hung up, the channel should NOT return the data
    /// back to the caller but SHOULD panic as hang ups are considered
    /// program logic errors.
    fn try_send(&self, data: T) -> Option<T>;
}

impl<T: Send> TrySender<T> for mpsc::Sender<T> {
    fn try_send(&self, data: T) -> Option<T> {
        self.send(data).expect("bip_util: mpsc::Sender Signaled A Hang Up");

        None
    }
}

impl<T: Send> TrySender<T> for mpsc::SyncSender<T> {
    fn try_send(&self, data: T) -> Option<T> {
        self.try_send(data).err().and_then(|err| {
            match err {
                TrySendError::Full(data) => Some(data),
                TrySendError::Disconnected(_) => panic!("bip_util: mpsc::SyncSender Signaled A Hang Up"),
            }
        })
    }
}
