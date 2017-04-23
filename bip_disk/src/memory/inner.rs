use std::default::Default;
use std::ops::{Deref, DerefMut};

use memory::block::BlockMetadata;

/// `InnerBlock` holding block data.
#[derive(Debug)]
pub struct InnerBlock {
    metadata: BlockMetadata,
    buffer:   Vec<u8>
}

impl InnerBlock {
    /// Create a new `InnerBlock` with a fixed length.
    pub fn new(len: usize) -> InnerBlock {
        InnerBlock{ metadata: BlockMetadata::default(), buffer: vec![0u8; len] }
    }

    /// Immutable access to the contained metadata.
    pub fn metadata(&self) -> &BlockMetadata {
        &self.metadata
    }

    /// Set the contained metadata.
    pub fn set_metadata(&mut self, metadata: BlockMetadata) {
        self.metadata = metadata;
    }
}

impl Deref for InnerBlock {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buffer
    }
}

impl DerefMut for InnerBlock {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
}