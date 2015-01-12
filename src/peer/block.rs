use peer::message::{PieceIndex, BlockOffset, BlockLength};

/// Represents a block of data within a piece. Many blocks make up a piece, many
/// pieces make up a file, and many (or one) file(s) make up the data being pointed
/// to by a torrent file.
pub struct Block {
    data:         Vec<u8>,
    active:       bool,
    piece_index:  PieceIndex,
    block_offset: BlockOffset,
    block_len:    BlockLength
}

impl Block {
    /// Creates a block with capacity size.
    pub fn with_capacity(capacity: BlockLength) -> Block {
        Block{ data: Vec::with_capacity(capacity as usize), active: false, 
            piece_index: -1, block_offset: -1, block_len: -1 }
    }
    
    /// Returns true if the current block is marked as active.
    pub fn is_active(&self) -> bool {
        self.active
    }
    
    /// Marks the current block as active and sets the block information passed in.
    pub fn set_active(&mut self, index: PieceIndex, offset: BlockOffset, length: BlockLength) {
        self.piece_index = index;
        self.block_offset = offset;
        self.block_len = length;
        
        self.active = true;
    }
    
    /// Marks the current block as inactive.
    pub fn set_inactive(&mut self) {
        self.active = false;
    }
    
    /// Returns a mutable slice to the data of this block.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }
    
    /// Returns the zero based piece index for the piece that this block belongs to.
    pub fn piece_index(&self) -> PieceIndex {
        self.piece_index
    }
    
    /// Returns the byte offset within the piece that this block belongs to.
    pub fn offset(&self) -> BlockOffset {
        self.block_offset
    }
    
    /// Returns the block length for the current block.
    pub fn len(&self) -> BlockLength {
        self.block_len
    }
}

impl AsSlice<u8> for Block {
    fn as_slice<'a>(&'a self) -> &'a [u8] {
        self.data.as_slice()
    }
}