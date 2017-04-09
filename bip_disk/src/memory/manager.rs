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