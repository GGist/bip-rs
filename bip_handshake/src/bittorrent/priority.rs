use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use bittorrent::handler::Task;

use mio::Sender;

pub struct PriorityChannel<T: Send> {
    send: Sender<Task<T>>,
    count: Arc<AtomicUsize>,
    capacity: usize,
}

impl<T: Send> PriorityChannel<T> {
    pub fn new(send: Sender<Task<T>>, capacity: usize) -> PriorityChannel<T> {
        PriorityChannel {
            send: send,
            count: Arc::new(AtomicUsize::new(0)),
            capacity: capacity,
        }
    }

    pub fn send(&self, task: Task<T>, prioritize: bool) {
        let should_send = self.try_count_increment(prioritize);

        if should_send {
            self.send
                .send(task)
                .expect("bip_handshake: BTHandshaker Failed To Send To The Handshaker Thread");
        }
    }

    pub fn channel_ack(&self) -> PriorityChannelAck {
        PriorityChannelAck { count: self.count.clone() }
    }

    fn try_count_increment(&self, prioritize: bool) -> bool {
        let our_count = self.count.fetch_add(1, Ordering::SeqCst);

        if prioritize || our_count <= self.capacity {
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

pub struct PriorityChannelAck {
    count: Arc<AtomicUsize>,
}

impl PriorityChannelAck {
    pub fn ack_task(&self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
    }
}
