use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::marker::PhantomData;

use sender::Sender;

/// `PrioritySender` allows non-blocking channels to forward both regular messages
/// and prioritized messages.
///
/// When a regular message would exceed the un-acked capacity of the sender they
/// will be returned back to the caller. If a prioritized message would exceed
/// the un-acked capacity of the sender, it will be sent anyway.
pub struct PrioritySender<S, T> {
    send: S,
    count: Arc<AtomicUsize>,
    capacity: usize,
    _unused: PhantomData<T>,
}

impl<S, T> PrioritySender<S, T>
    where S: Sender<T>,
          T: Send
{
    /// Create a new `PrioritySender`.
    ///
    /// It is advisable to set the capacity to SENDER_CAPACITY - PRIORITY_CAPACITY
    /// so that when you send a prioritized message, you don't overflow the actual
    /// capacity of the sender.
    ///
    /// It is important that priority messages are NOT acked. It is suggested that
    /// you can infer priority data by inspecting the type T in some manner.
    pub fn new(send: S, capacity: usize) -> PrioritySender<S, T> {
        PrioritySender {
            send: send,
            count: Arc::new(AtomicUsize::new(0)),
            capacity: capacity,
            _unused: PhantomData,
        }
    }

    /// Attempt to send data through the underlying sender.
    pub fn send(&self, data: T) -> Option<T> {
        let should_send = self.try_count_increment(false);

        if should_send {
            self.send.send(data);
            None
        } else {
            Some(data)
        }
    }

    /// Send prioritized data through the underlying sender.
    pub fn prioritized_send(&self, data: T) {
        self.try_count_increment(true);

        self.send.send(data);
    }

    /// Create a new PrioritySenderAck that can be used to ack messages sent by this
    /// PrioritySender.
    pub fn sender_ack(&self) -> PrioritySenderAck {
        PrioritySenderAck { count: self.count.clone() }
    }

    fn try_count_increment(&self, prioritize: bool) -> bool {
        let our_count = self.count.fetch_add(1, Ordering::SeqCst);

        if prioritize || our_count < self.capacity {
            // If we are prioritizing, we ignore the current capacity
            // If our count is equal to capacity, we can still go ahead
            true
        } else {
            // Failed to get a passable count, revert our add
            self.count.fetch_sub(1, Ordering::SeqCst);
            false
        }
    }
}

// ----------------------------------------------------------------------------//

/// `PrioritySenderAck` allows a client to ack messages received from a `PrioritySender`.
pub struct PrioritySenderAck {
    count: Arc<AtomicUsize>,
}

impl PrioritySenderAck {
    /// Ack a message received from a PrioritySender.
    ///
    /// Priority messages should NOT be acked.
    pub fn ack(&self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use super::PrioritySender;

    #[test]
    fn positive_send_priority_zero_capacity() {
        let (send, recv) = mpsc::channel();
        let priority_send = PrioritySender::new(send, 0);

        priority_send.prioritized_send(());
        assert!(recv.try_recv().is_ok());
    }

    #[test]
    fn negative_send_zero_capacity() {
        let (send, recv) = mpsc::channel();
        let priority_send = PrioritySender::new(send, 0);

        assert_eq!(priority_send.send(()), Some(()));
        assert!(recv.try_recv().is_err());
    }
}
