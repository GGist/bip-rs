use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::send::TrySender;

/// Create two SplitSenders over a single Sender with corresponding capacities.
pub fn split_sender<S, T>(
    send: S,
    cap_one: usize,
    cap_two: usize,
) -> (SplitSender<S>, SplitSender<S>)
where
    S: TrySender<T> + Clone,
    T: Send,
{
    (
        SplitSender::new(send.clone(), cap_one),
        SplitSender::new(send, cap_two),
    )
}

/// SplitSender allows dividing the capacity of a single channel into multiple
/// channels.
pub struct SplitSender<S> {
    send: S,
    count: Arc<AtomicUsize>,
    capacity: usize,
}

impl<S> Clone for SplitSender<S>
where
    S: Clone,
{
    fn clone(&self) -> SplitSender<S> {
        SplitSender {
            send: self.send.clone(),
            count: self.count.clone(),
            capacity: self.capacity,
        }
    }
}

unsafe impl<S> Sync for SplitSender<S> where S: Sync {}

impl<S> SplitSender<S> {
    /// Create a new `SplitSender`.
    pub fn new(send: S, capacity: usize) -> SplitSender<S> {
        SplitSender {
            send: send,
            count: Arc::new(AtomicUsize::new(0)),
            capacity: capacity,
        }
    }

    /// Create a new SplitSenderAck that can be used to ack sent messages.
    pub fn sender_ack(&self) -> SplitSenderAck {
        SplitSenderAck {
            count: self.count.clone(),
        }
    }

    fn try_count_increment(&self) -> bool {
        let our_count = self.count.fetch_add(1, Ordering::SeqCst);

        if our_count < self.capacity {
            true
        } else {
            // Failed to get a passable count, revert our add
            self.count.fetch_sub(1, Ordering::SeqCst);

            false
        }
    }
}

impl<S, T> TrySender<T> for SplitSender<S>
where
    S: TrySender<T>,
    T: Send,
{
    fn try_send(&self, data: T) -> Option<T> {
        let should_send = self.try_count_increment();

        if should_send {
            self.send.try_send(data)
        } else {
            Some(data)
        }
    }
}

// ---------------------------------------------------------------------------//

/// `SplitSenderAck` allows a client to ack messages received from a
/// `SplitSender`.
pub struct SplitSenderAck {
    count: Arc<AtomicUsize>,
}

impl SplitSenderAck {
    /// Ack a message received from a SplitSender.
    pub fn ack(&self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::SplitSender;
    use crate::send::TrySender;

    #[test]
    fn positive_send_zero_capacity() {
        let (send, recv) = mpsc::channel();
        let split_sender = SplitSender::new(send, 0);

        assert!(split_sender.try_send(()).is_some());
        assert!(recv.try_recv().is_err());
    }

    #[test]
    fn positive_send_one_capacity() {
        let (send, recv) = mpsc::channel();
        let split_sender = SplitSender::new(send, 1);

        assert!(split_sender.try_send(()).is_none());
        assert!(recv.try_recv().is_ok());
    }
}
