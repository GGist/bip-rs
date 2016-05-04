use std::borrow::{Cow};

use nom::{IResult, be_u32};

const BITS_PER_BYTE: u32 = 8;

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

pub struct BitFieldMessage<'a> {
    bytes: Cow<'a, [u8]>
}

impl<'a> BitFieldMessage<'a> {
    pub fn new(total_pieces: u32) -> BitFieldMessage<'a> {
        // total_pieces is the number of bits we expect
        let bytes_needed = if total_pieces % BITS_PER_BYTE == 0 {
            total_pieces / BITS_PER_BYTE
        } else {
            (total_pieces / BITS_PER_BYTE) + 1
        };
        
        BitFieldMessage{ bytes: Cow::Owned(vec![0u8; bytes_needed as usize]) }
    }
    
    pub fn from_bytes(bytes: &'a [u8], len: u32) -> IResult<&'a [u8], BitFieldMessage<'a>> {
        parse_bitfield(bytes, len)
    }
}

fn parse_bitfield<'a>(bytes: &'a [u8], len: u32) -> IResult<&'a [u8], BitFieldMessage<'a>> {
    map!(bytes, take!(len as usize), |bytes| BitFieldMessage{ bytes: Cow::Borrowed(bytes) })
}

//----------------------------------------------------------------------------//

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

pub struct PieceMessage<'a> {
    piece_index:  u32,
    block_offset: u32,
    block_data:   Cow<'a, [u8]>
}

impl<'a> PieceMessage<'a> {
    pub fn new(piece_index: u32, block_offset: u32, block_data: &'a [u8]) -> PieceMessage<'a> {
        PieceMessage{ piece_index: piece_index, block_offset: block_offset,
            block_data: Cow::Borrowed(block_data) }
    }
    
    pub fn from_bytes(bytes: &'a [u8], len: u32) -> IResult<&'a [u8], PieceMessage<'a>> {
        parse_piece(bytes, len)
    }
}

fn parse_piece<'a>(bytes: &'a [u8], len: u32) -> IResult<&'a [u8], PieceMessage<'a>> {
    chain!(bytes,
        piece_index:  be_u32 ~
        block_offset: be_u32 ~
        block_data:   take!(len as usize) ,
        || { PieceMessage::new(piece_index, block_offset, block_data) }
    )
}

//----------------------------------------------------------------------------//

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