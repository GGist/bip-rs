use std::ops::{Deref, DerefMut};

use bip_util::bt::{self, InfoHash};

use bytes::{Bytes, BytesMut};

//----------------------------------------------------------------------------//

/// `BlockMetadata` which tracks metadata associated with a `Block` of memory.
#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub struct BlockMetadata {
    info_hash:    InfoHash,
    piece_index:  u64,
    block_offset: u64,
    block_length: usize
}

impl BlockMetadata {
    pub fn new(info_hash: InfoHash, piece_index: u64, block_offset: u64, block_length: usize) -> BlockMetadata {
        BlockMetadata{ info_hash, piece_index,
                       block_offset, block_length }
    }

    pub fn with_default_hash(piece_index: u64, block_offset: u64, block_length: usize) -> BlockMetadata {
        BlockMetadata::new([0u8; bt::INFO_HASH_LEN].into(), piece_index, block_offset, block_length)
    }

    pub fn info_hash(&self) -> InfoHash {
        self.info_hash
    }

    pub fn piece_index(&self) -> u64 {
        self.piece_index
    }

    pub fn block_offset(&self) -> u64 {
        self.block_offset
    }

    pub fn block_length(&self) -> usize {
        self.block_length
    }
}

impl Default for BlockMetadata {
    fn default() -> BlockMetadata {
        BlockMetadata::new([0u8; bt::INFO_HASH_LEN].into(), 0, 0, 0)
    }
}

//----------------------------------------------------------------------------//

/// `Block` of immutable memory.
#[derive(Debug)]
pub struct Block {
    metadata:   BlockMetadata,
    block_data: Bytes
}

impl Block {
    /// Create a new `Block`.
    pub fn new(metadata: BlockMetadata, block_data: Bytes) -> Block {
        Block{ metadata, block_data }
    }

    /// Access the metadata for the block.
    pub fn metadata(&self) -> BlockMetadata {
        self.metadata
    }

    pub fn into_parts(self) -> (BlockMetadata, Bytes) {
        (self.metadata, self.block_data)
    }
}

impl From<BlockMut> for Block {
    fn from(block: BlockMut) -> Block {
        Block::new(block.metadata(), block.block_data.freeze())
    }
}

impl Deref for Block {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.block_data
    }
}

//----------------------------------------------------------------------------//

/// `BlockMut` of mutable memory.
#[derive(Debug)]
pub struct BlockMut {
    metadata:   BlockMetadata,
    block_data: BytesMut
}

impl BlockMut {
    /// Create a new `BlockMut`.
    pub fn new(metadata: BlockMetadata, block_data: BytesMut) -> BlockMut {
        BlockMut{ metadata, block_data }
    }

    /// Access the metadata for the block.
    pub fn metadata(&self) -> BlockMetadata {
        self.metadata
    }

    pub fn into_parts(self) -> (BlockMetadata, BytesMut) {
        (self.metadata, self.block_data)
    }
}

impl Deref for BlockMut {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.block_data
    }
}

impl DerefMut for BlockMut {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.block_data
    }
}