use std::cmp;
use std::sync::{RwLock, Mutex};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use bip_util::contiguous::{ContiguousBuffer, ContiguousBuffers};
use crossbeam::sync::MsQueue;

use token::Token;

// Always hold a minimum number of blocks, but if all blocks are in use, we
// might as well spend some time allocating some more, up to a certain point,
// but don't throw them all back to the free queue, only up to the free count.
const MAX_FREE_COUNT_SIZE: usize = 100;   // With 16KB block size, we are at ~1.6MB
const MAX_TOTAL_COUNT_SIZE: usize = 1000; // With 16KB block size, we are at ~15.6MB

/// Thread safe storage for re-usable blocks of memory that can be combined or broken up.
struct Blocks {
    free:        MsQueue<ContiguousBuffers<Vec<u8>>>,
    used:        RwLock<HashMap<Token, Mutex<HashMap<Token, ContiguousBuffers<Vec<u8>>>>>>,
    free_count:  AtomicUsize,
    total_count: AtomicUsize,
    block_size:  usize
}

impl Blocks {
    /// Create a new chunk of pre-allocated block sized regions of memory.
    pub fn new(block_size: usize) -> Blocks {
        if block_size == 0 {
            panic!("bip_peer: Blocks Created With A Block Size Of 0 Not Allowed")
        }

        let mut empty_blocks = Blocks {
            free: MsQueue::new(),
            used: RwLock::new(HashMap::new()),
            free_count: AtomicUsize::new(0),
            total_count: AtomicUsize::new(0),
            block_size: block_size
        };

        for _ in 0..MAX_FREE_COUNT_SIZE {
            let free_block = empty_blocks.allocate_contiguous_blocks(1);
            empty_blocks.reuse_blocks(free_block);
        }

        empty_blocks
    }

    /// Register's a new namespace (defined as a token) with the Blocks structure.
    pub fn register_namespace(&self, namespace: Token) {
        self.run_with_namespace_map(|namespace_map| {
            if namespace_map.insert(namespace, Mutex::new(HashMap::new())).is_some() {
                panic!("bip_peer: Blocks::register_namespace Found Existing Token");
            }
        });
    }

    /// Unregister's an existing namespace (defined as a token) from the Blocks structure.
    ///
    /// This call will implicitly reclaim any active used blocks under the namespace.
    pub fn unregister_namespace(&self, namespace: Token) {
        self.run_with_namespace_map(|namespace_map| {
            if namespace_map.remove(&namespace).is_none() {
                panic!("bip_peer: Blocks::unregister_namespace Failed To Remove Existing Token");
            }
        });
    }

    /// Allocate a block with AT LEAST the given number of bytes under the namespace and
    /// assocaited with the given request id (token).
    ///
    /// This function will block until the block can be reserved.
    pub fn allocate_block(&self, namespace: Token, request: Token, num_bytes: usize) {
        let blocks_required = self.calcualte_blocks_required(num_bytes);
        let contiguous_block = self.allocate_contiguous_blocks(blocks_required);

        self.add_used_block(namespace, request, contiguous_block);
    }

    /// Access a block under the given namespace, corresponding to the given request id.
    pub fn access_block<F>(&self, namespace: Token, request: Token, access: F)
        where F: FnOnce(&mut ContiguousBuffers<Vec<u8>>) {
        self.run_with_request_map(namespace, |request_map| {
            let contiguous_block = request_map.get_mut(&request)
                .expect("bip_peer: Blocks::access_block Failed To Find Request For Id");

            access(contiguous_block);
        });
    }

    /// Reclaim a block under the given namespace, corresponding to the given request id.
    pub fn reclaim_block(&self, namespace: Token, request: Token) {
        self.remove_used_block(namespace, request);
    }

    // ----- PRIVATE ----- //

    /// Run the given closure with a mutable reference to the namespace map.
    fn run_with_namespace_map<F>(&self, accept: F)
        where F: FnOnce(&mut HashMap<Token, Mutex<HashMap<Token, ContiguousBuffers<Vec<u8>>>>>) {
        let mut namespace_map = self.used.write()
            .expect("bip_peer: Blocks::run_with_request_map Failed To Read From Used Map");

        accept(&mut namespace_map);
    }

    /// Run the given closure with a mutable reference to the request map under the given namespace.
    fn run_with_request_map<F>(&self, namespace: Token, accept: F)
        where F: FnOnce(&mut HashMap<Token, ContiguousBuffers<Vec<u8>>>) {
        let namespace_map = self.used.read()
            .expect("bip_peer: Blocks::run_with_request_map Failed To Read From Used Map");
        let mut request_map = namespace_map.get(&namespace)
            .expect("bip_peer: Blocks::run_with_request_map Failed To Find Map For Namespace").lock()
            .expect("bip_peer: Blocks::run_with_request_map Failed To Lock Request Map");

        accept(&mut request_map);
    }

    /// Add the given blocks to the used map under the given namespace and request id.
    fn add_used_block(&self, namespace: Token, request: Token, blocks: ContiguousBuffers<Vec<u8>>) {
        self.run_with_request_map(namespace, |request_map| {
            if request_map.insert(request, blocks).is_some() {
                panic!("bip_peer: Blocks::add_used_block Saw Block With Duplicate Request Id Token")
            }
        });
    }

    /// Remove the given blocks to the used map under the given namespace and request id.
    fn remove_used_block(&self, namespace: Token, request: Token) {
        self.run_with_request_map(namespace, |request_map| {
            if request_map.remove(&request).is_none() {
                panic!("bip_peer: Blocks::remove_used_block Failed To Remove Existing Block For Request Id")
            }
        });
    }

    /// Try to re-use the given block by pushing it to the free queue if there is space.
    fn reuse_blocks(&self, block: ContiguousBuffers<Vec<u8>>) {
        // See if we can increment our atomic so the old value is under the maximum
        let old_free_size = self.free_count.fetch_add(1, Ordering::SeqCst);

        if old_free_size < MAX_FREE_COUNT_SIZE {
            self.free.push(block);
        } else {
            // Can't put this back in the free queue, re decrement the free count and decrement the total count as well
            self.free_count.fetch_sub(1, Ordering::SeqCst);
            self.total_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Calculate the number of blocks required to meet the total size requested.
    fn calcualte_blocks_required(&self, total_size: usize) -> usize {
        let whole_blocks = total_size / self.block_size;

        if total_size % self.block_size == 0 { whole_blocks } else { whole_blocks + 1 }
    }

    /// Allocate blocks_required number of contiguous blocks.
    fn allocate_contiguous_blocks(&self, mut blocks_required: usize) -> ContiguousBuffers<Vec<u8>> {
        let mut buffers = ContiguousBuffers::new();

        while blocks_required != 0 {
            // Try to non blocking allocate from free queue, else from just allocating
            // it ourselves (if we havent reached our limit), else block on free queue
            let buffer = self.non_block_allocate_from_free_queue()
                .or_else(|| self.allocate_from_allocator())
                .unwrap_or_else(|| self.allocate_from_free_queue());
                
            buffers.pack(buffer);

            blocks_required -= 1;
        }

        buffers
    }

    /// Try to allocate a block from the free queue without blocking.
    fn non_block_allocate_from_free_queue(&self) -> Option<ContiguousBuffers<Vec<u8>>> {
        self.free.try_pop().and_then(|block| {
            // Decrement other free count since we removed a block from it
            self.free_count.fetch_sub(1, Ordering::SeqCst);
            
            Some(block)
        })
    }

    /// Try to allocate a block from the standard allocator if we have room.
    fn allocate_from_allocator(&self) -> Option<ContiguousBuffers<Vec<u8>>> {
        // Try to increment total count, see if the old value was less than max blocks
        let old_value = self.total_count.fetch_add(1, Ordering::SeqCst);

        if old_value < MAX_TOTAL_COUNT_SIZE {
            // We got the block, go ahead and allocate it
            Some(allocate_contiguous_block(self.block_size))
        } else {
            // We didn't get the block, subtract back what we added
            self.total_count.fetch_sub(1, Ordering::SeqCst);
            None
        }
    }

    /// Allocate a block from the free queue by blocking if necessary.
    fn allocate_from_free_queue(&self) -> ContiguousBuffers<Vec<u8>> {
        let block = self.free.pop();

        // Decrement othe free count since we removed a block from it
        self.free_count.fetch_sub(1, Ordering::SeqCst);

        block
    }
}

/// Allocate a contiguous buffer with a single buffer of capacity buffer_size.
fn allocate_contiguous_block(buffer_size: usize) -> ContiguousBuffers<Vec<u8>> {
    ContiguousBuffers::with_buffer(Vec::with_capacity(buffer_size))
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::sync::mpsc;
    use std::time::Duration;

    use bip_util::contiguous::ContiguousBuffer;
    
    use super::Blocks;
    use token::{Token, TokenGenerator};

    //pub fn allocate_block(&self, namespace: Token, request: Token, num_bytes: usize) {

/*
/// Register's a new namespace (defined as a token) with the Blocks structure.
    pub fn register_namespace(&self, namespace: Token) {
        self.run_with_namespace_map(|namespace_map| {
            if namespace_map.insert(namespace, Mutex::new(HashMap::new())).is_some() {
                panic!("bip_peer: Blocks::register_namespace Found Existing Token");
            }
        });
    }

    /// Unregister's an existing namespace (defined as a token) from the Blocks structure.
    ///
    /// This call will implicitly reclaim any active used blocks under the namespace.
    pub fn unregister_namespace(&self, namespace: Token) {
        self.run_with_namespace_map(|namespace_map| {
            if namespace_map.remove(&namespace).is_none() {
                panic!("bip_peer: Blocks::unregister_namespace Failed To Remove Existing Token");
            }
        });
    }

    /// Allocate a block with AT LEAST the given number of bytes under the namespace and
    /// assocaited with the given request id (token).
    ///
    /// This function will block until the block can be reserved.
    pub fn allocate_block(&self, namespace: Token, request: Token, num_bytes: usize) {
        let blocks_required = self.calcualte_blocks_required(num_bytes);
        let contiguous_block = self.allocate_contiguous_blocks(blocks_required);

        self.add_used_block(namespace, request, contiguous_block);
    }

    /// Access a block under the given namespace, corresponding to the given request id.
    pub fn access_block<F>(&self, namespace: Token, request: Token, access: F)
        where F: FnOnce(&mut ContiguousBuffers<Vec<u8>>) {
        self.run_with_request_map(namespace, |request_map| {
            let contiguous_block = request_map.get_mut(&request)
                .expect("bip_peer: Blocks::access_block Failed To Find Request For Id");

            access(contiguous_block);
        });
    }

    /// Reclaim a block under the given namespace, corresponding to the given request id.
    pub fn reclaim_block(&self, namespace: Token, request: Token) {
        self.remove_used_block(namespace, request);
    }
*/

    #[test]
    fn positive_create_blocks_non_zero_block_size() {
        Blocks::new(1024);
    }

    #[test]
    #[should_panic]
    fn negative_create_blocks_zero_block_size() {
        Blocks::new(0);
    }

    #[test]
    fn positive_register_namespace() {
        let blocks = Blocks::new(1);
        let mut generator = TokenGenerator::new();
        
        blocks.register_namespace(generator.generate());
    }

    #[test]
    #[should_panic]
    fn negative_register_namespace_duplicate() {
        let blocks = Blocks::new(1);
        let mut generator = TokenGenerator::new();
        
        let namespace = generator.generate();
        blocks.register_namespace(namespace);
        blocks.register_namespace(namespace);
    }

    #[test]
    fn positive_unregister_namespace() {
        let blocks = Blocks::new(1);
        let mut generator = TokenGenerator::new();
        
        let namespace = generator.generate();
        blocks.register_namespace(namespace);
        blocks.unregister_namespace(namespace);
    }

    #[test]
    fn positive_unregister_namespace_reregister_namespace() {
        let blocks = Blocks::new(1);
        let mut generator = TokenGenerator::new();
        
        let namespace = generator.generate();
        blocks.register_namespace(namespace);
        blocks.unregister_namespace(namespace);
        blocks.register_namespace(namespace);
    }

    #[test]
    #[should_panic]
    fn negative_unregister_namespace_duplicate() {
        let blocks = Blocks::new(1);
        let mut generator = TokenGenerator::new();
        
        let namespace = generator.generate();
        blocks.register_namespace(namespace);
        blocks.unregister_namespace(namespace);
        blocks.unregister_namespace(namespace);
    }

    #[test]
    fn positive_allocate_block_zero_bytes() {
        let blocks = Blocks::new(1);
        let mut generator = TokenGenerator::new();
        
        let namespace = generator.generate();
        blocks.register_namespace(namespace);
        
        let request = generator.generate();
        blocks.allocate_block(namespace, request, 0);

        blocks.access_block(namespace, request, |contiguous_buffers| {
            assert_eq!(0, contiguous_buffers.capacity());
        });
    }

    #[test]
    fn positive_allocate_block_non_zero_bytes() {
        let blocks = Blocks::new(100);
        let mut generator = TokenGenerator::new();
        
        let namespace = generator.generate();
        blocks.register_namespace(namespace);
        
        let request = generator.generate();
        blocks.allocate_block(namespace, request, 500);

        blocks.access_block(namespace, request, |contiguous_buffers| {
            assert!(contiguous_buffers.capacity() >= 500);
        });
    }

    #[test]
    fn positive_allocate_blocks_maximum_used_blocks() {
        let blocks = Blocks::new(1);
        let mut generator = TokenGenerator::new();

        let (send, recv) = mpsc::channel();
        thread::spawn(move || {
            let namespace = generator.generate();
            blocks.register_namespace(namespace);

            loop {
                blocks.allocate_block(namespace, generator.generate(), 1);
                send.send(());
            }
        });
        thread::sleep(Duration::from_millis(100));

        let mut count = 0;
        while let Ok(_) = recv.try_recv() {
            count += 1;
        }

        assert_eq!(count, super::MAX_TOTAL_COUNT_SIZE);
    }

    #[test]
    #[should_panic]
    fn negative_allocate_block_unregistered_namespace() {
        let blocks = Blocks::new(100);
        let mut generator = TokenGenerator::new();

        blocks.allocate_block(generator.generate(), generator.generate(), 1024);
    }

    #[test]
    fn positive_access_block_read_and_write() {
        let blocks = Blocks::new(1);
        let mut generator = TokenGenerator::new();
        
        let namespace = generator.generate();
        blocks.register_namespace(namespace);
        
        let request = generator.generate();
        blocks.allocate_block(namespace, request, 10);

        let mut read_bytes = Vec::new();
        blocks.access_block(namespace, request, |contiguous_buffers| {
            contiguous_buffers.write(b"testing");

            contiguous_buffers.read(|bytes| {
                read_bytes.extend_from_slice(bytes);
            })
        });

        assert_eq!(b"testing", &read_bytes[..]);
    }

    #[test]
    fn positive_access_block_read_and_write_two_calls() {
        let blocks = Blocks::new(1);
        let mut generator = TokenGenerator::new();
        
        let namespace = generator.generate();
        blocks.register_namespace(namespace);
        
        let request = generator.generate();
        blocks.allocate_block(namespace, request, 10);

        let mut read_bytes = Vec::new();
        blocks.access_block(namespace, request, |contiguous_buffers| {
            contiguous_buffers.write(b"testing");
        });

        blocks.access_block(namespace, request, |contiguous_buffers| {
            contiguous_buffers.read(|bytes| {
                read_bytes.extend_from_slice(bytes);
            })
        });

        assert_eq!(b"testing", &read_bytes[..]);
    }
}