use std::sync::Arc;

use memory::inner::InnerBlock;
use memory::block::{self, Block};

use crossbeam::sync::TreiberStack;
use futures::{Poll, Async};
use futures::stream::Stream;

/// `BlockManager` object that manages a number of `Block` objects.
pub struct BlockManager {
    blocks: Arc<TreiberStack<InnerBlock>>
}

impl BlockManager {
    /// Create new `BlockManager` with the given number of blocks and block size.
    pub fn new(num_blocks: usize, block_len: usize) -> BlockManager {
        let blocks = Arc::new(TreiberStack::new());

        for _ in 0..num_blocks {
            blocks.push(InnerBlock::new(block_len));
        }

        BlockManager{ blocks: blocks }
    }
}

impl Stream for BlockManager {
    type Item = Block;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Block>, ()> {
        self.blocks.try_pop().map(|inner| {
            Ok(Async::Ready(Some(block::new_block(inner, self.blocks.clone()))))
        }).unwrap_or(Ok(Async::NotReady))
    }
}

#[cfg(test)]
mod tests {
    use super::BlockManager;

    use std::mem;

    use futures::stream::Stream;

    #[test]
    fn positive_wait_single_block() {
        let mut manager = BlockManager::new(1, 1).wait();
        let block = manager.next().unwrap().unwrap();

        assert_eq!(block.len(), 1);
    }

    #[test]
    fn positive_reclaim_single_block() {
        let mut manager = BlockManager::new(1, 1).wait();

        let block = manager.next().unwrap().unwrap();
        mem::drop(block);
        let block = manager.next().unwrap().unwrap();

        assert_eq!(block.len(), 1);
    }

    #[test]
    fn positive_wait_many_blocks() {
        let mut manager = BlockManager::new(2, 1).wait();
        let block_one = manager.next().unwrap().unwrap();
        let block_two = manager.next().unwrap().unwrap();

        assert_eq!(block_one.len(), 1);
        assert_eq!(block_two.len(), 1);
    }

    #[test]
    fn positive_reclaim_many_blocks() {
        let mut manager = BlockManager::new(2, 1).wait();

        let block_one = manager.next().unwrap().unwrap();
        let block_two = manager.next().unwrap().unwrap();
        mem::drop(block_one);
        mem::drop(block_two);
        let block_one = manager.next().unwrap().unwrap();
        let block_two = manager.next().unwrap().unwrap();

        assert_eq!(block_one.len(), 1);
        assert_eq!(block_two.len(), 1);
    }
}