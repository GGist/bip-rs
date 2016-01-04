use std::sync::{RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Holds an index yielded by the IndexQueue.
///
/// This item may be put back into the queue if desired.
pub struct Index {
    index: usize
}

impl Index {
    fn new(index: usize) -> Index {
        Index{ index: index }
    }
    
    /// Get the usize equivalent of the Index.
    pub fn get(&self) -> usize {
        self.index
    }
}

/// Concurrent implementation of an index queue.
///
/// Consumers can concurrently request indices that have not been exhausted and use
/// them in computations. If an index is found to not be ready for processing, consumers
/// can put that index back to be used before new indices are yielded.
pub struct IndexQueue {
    main_queue: AtomicUsize,
    side_queue: RwLock<Vec<usize>>
}

impl IndexQueue {
    /// Create a new IndexQueue starting at 0.
    pub fn new() -> IndexQueue {
        IndexQueue{ main_queue: AtomicUsize::new(0), side_queue: RwLock::new(Vec::new()) }
    }
    
    /// Get the next Index from the IndexQueue.
    pub fn get(&self) -> Index {
        // Check if we have an index that was put back
        // Optomize for the common case where the size queue is empty, check via a read
        let get_from_side_queue = !self.side_queue.read().unwrap().is_empty();
        
        let opt_index = if get_from_side_queue {
            // Acquire a write lock and try to pop the last index, another thread may have beat us!!!
            self.side_queue.write().unwrap().pop()
        } else {
            None
        };
        
        match opt_index {
            Some(side_index) => Index::new(side_index),
            None             => {
                let new_index = self.main_queue.fetch_add(1, Ordering::AcqRel);
                
                Index::new(new_index)
            }
        }
    }
    
    /// Put back an Index that was taken from the IndexQueue.
    ///
    /// This index will be yielded before any new indices are created.
    /// It MAY not be the next index yielded if another index was added afterwards.
    pub fn put_back(&self, index: Index) {
        self.side_queue.write().unwrap().push(index.get());
    }
}