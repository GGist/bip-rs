use std::sync::{Arc};
use std::mem;
use std::ops::{Deref, DerefMut};

use memory::inner::InnerBlock;

use bip_util::bt::{self, InfoHash};
use crossbeam::sync::TreiberStack;
use futures::sync::mpsc::Sender;
use futures::sink::{Sink, Wait};
use futures::Future;

//----------------------------------------------------------------------------//

/// `BlockMetadata` which tracks metadata associated with a `Block` of memory.
pub struct BlockMetadata {
    info_hash:    InfoHash,
    piece_index:  u64,
    block_offset: u64,
    block_length: usize
}

impl Default for BlockMetadata {
    fn default() -> BlockMetadata {
        BlockMetadata{ info_hash: [0u8; bt::INFO_HASH_LEN].into(), piece_index: 0,
                   block_offset: 0, block_length: 0 }
    }
}

//----------------------------------------------------------------------------//

/// `Block` of memory which is tracked by the underlying `MemoryManager`.
pub struct Block {
    inner: InnerBlock,
    free:  Arc<TreiberStack<InnerBlock>>
}

/// Create a new `Block` of memory from the given arguments.
pub fn new_block(inner: InnerBlock, free: Arc<TreiberStack<InnerBlock>>) -> Block {
    Block{ inner: inner, free: free }
}

impl Block {
    /// Access the metadata for the block.
    pub fn metadata(&self) -> &BlockMetadata {
        self.inner.metadata()
    }

    /// Set the metadata for the block.
    pub fn set_metadata(&mut self, metadata: BlockMetadata) {
        self.inner.set_metadata(metadata)
    }
}

impl Deref for Block {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        Deref::deref(&self.inner)
    }
}

impl DerefMut for Block {
    fn deref_mut(&mut self) -> &mut [u8] {
        DerefMut::deref_mut(&mut self.inner)
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        // Swap in an empty InnerBlock so we can push ours back to the stack
        let inner = mem::replace(&mut self.inner, InnerBlock::new(0));

        self.free.push(inner);
    }
}