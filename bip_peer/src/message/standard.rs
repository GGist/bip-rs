use std::borrow::{Cow};

use nom::{IResult, be_u32};

const BITS_PER_BYTE: u32 = 8;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct HaveMessage {
    piece_index: u32
}

impl HaveMessage {
    pub fn new(piece_index: u32) -> HaveMessage {
        HaveMessage{ piece_index: piece_index }
    }
    
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], HaveMessage> {
        parse_have(bytes)
    }
    
    pub fn piece_index(&self) -> u32 {
        self.piece_index
    }
}

fn parse_have(bytes: &[u8]) -> IResult<&[u8], HaveMessage> {
    map!(bytes, be_u32, |index| HaveMessage::new(index))
}

//----------------------------------------------------------------------------//

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct BitfieldMessage {
    num_bytes: u32
}

impl BitfieldMessage {
    pub fn new(num_bytes: u32) -> BitfieldMessage {
        BitfieldMessage{ num_bytes: num_bytes }
    }
    
    pub fn from_pieces(num_pieces: u32) -> BitfieldMessage {
        // num_pieces is the number of bits we expect
        let bytes_needed = if num_pieces % BITS_PER_BYTE == 0 {
            num_pieces / BITS_PER_BYTE
        } else {
            (num_pieces / BITS_PER_BYTE) + 1
        };
        
        BitfieldMessage::new(bytes_needed)
    }
}

//----------------------------------------------------------------------------//

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct RequestMessage {
    piece_index:  u32,
    block_offset: u32,
    block_length: u32
}

impl RequestMessage {
    pub fn new(piece_index: u32, block_offset: u32, block_length: u32) -> RequestMessage {
        RequestMessage{ piece_index: piece_index, block_offset: block_offset,
            block_length: block_length }
    }
    
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], RequestMessage> {
        parse_request(bytes)
    }
}

fn parse_request(bytes: &[u8]) -> IResult<&[u8], RequestMessage> {
    map!(bytes, tuple!(be_u32, be_u32, be_u32), |(index, offset, length)| {
        RequestMessage::new(index, offset, length)
    })
}

//----------------------------------------------------------------------------//

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct PieceMessage {
    piece_index:  u32,
    block_offset: u32,
    block_length: u32
}

impl PieceMessage {
    pub fn new(piece_index: u32, block_offset: u32, block_length: u32) -> PieceMessage {
        PieceMessage{ piece_index: piece_index, block_offset: block_offset, block_length: block_length }
    }
    
    pub fn from_bytes(bytes: &[u8], len: u32) -> IResult<&[u8], PieceMessage> {
        parse_piece(bytes, len)
    }
}

fn parse_piece(bytes: &[u8], len: u32) -> IResult<&[u8], PieceMessage> {
    chain!(bytes,
        piece_index:  be_u32 ~
        block_offset: be_u32 ~
        block_length: value!(len) ,
        || { PieceMessage::new(piece_index, block_offset, block_length) }
    )
}

//----------------------------------------------------------------------------//

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct CancelMessage {
    piece_index:  u32,
    block_offset: u32,
    block_length: u32
}

impl CancelMessage {
    pub fn new(piece_index: u32, block_offset: u32, block_length: u32) -> CancelMessage {
        CancelMessage{ piece_index: piece_index, block_offset: block_offset,
            block_length: block_length }
    }
    
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], CancelMessage> {
        parse_cancel(bytes)
    }
}

fn parse_cancel(bytes: &[u8]) -> IResult<&[u8], CancelMessage> {
    map!(bytes, tuple!(be_u32, be_u32, be_u32), |(index, offset, length)| {
        CancelMessage::new(index, offset, length)
    })
}