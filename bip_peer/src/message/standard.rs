use std::borrow::ToOwned;
use std::io::{self, Write};

use bytes::{Bytes, BytesMut};
use byteorder::{WriteBytesExt, BigEndian};
use nom::{IResult, be_u32, Needed};

use message;

const BITS_PER_BYTE: u32 = 8;

/// Message for notifying a peer of a piece that you have.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct HaveMessage {
    piece_index: u32,
}

impl HaveMessage {
    pub fn new(piece_index: u32) -> HaveMessage {
        HaveMessage { piece_index: piece_index }
    }

    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<HaveMessage>> {
        throwaway_input!(parse_have(bytes.as_ref()))
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        try!(message::write_length_id_pair(&mut writer, message::HAVE_MESSAGE_LEN, Some(message::HAVE_MESSAGE_ID)));

        writer.write_u32::<BigEndian>(self.piece_index)
    }

    pub fn piece_index(&self) -> u32 {
        self.piece_index
    }
}

fn parse_have(bytes: &[u8]) -> IResult<&[u8], io::Result<HaveMessage>> {
    map!(bytes, be_u32, |index| Ok(HaveMessage::new(index)))
}

// ----------------------------------------------------------------------------//

/// Message for notifying a peer of all of the pieces you have.
///
/// This should be sent immediately after the handshake.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct BitFieldMessage {
    bytes: Bytes
}

impl BitFieldMessage {
    pub fn new(bytes: Bytes) -> BitFieldMessage {
        BitFieldMessage { bytes: bytes }
    }

    pub fn parse_bytes(_input: (), mut bytes: Bytes, len: u32) -> IResult<(), io::Result<BitFieldMessage>> {
        let cast_len = message::u32_to_usize(len);

        if bytes.len() >= cast_len {
            IResult::Done((), Ok(BitFieldMessage{ bytes: bytes.split_to(cast_len) }))
        } else {
            IResult::Incomplete(Needed::Size(cast_len - bytes.len()))
        }
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        let actual_length = (1 + self.bytes.len()) as u32;
        try!(message::write_length_id_pair(&mut writer, actual_length, Some(message::BITFIELD_MESSAGE_ID)));

        writer.write_all(&self.bytes)
    }

    pub fn bitfield(&self) -> &[u8] {
        &self.bytes[..]
    }
}

// ----------------------------------------------------------------------------//

/// Message for requesting a block from a peer.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct RequestMessage {
    piece_index: u32,
    block_offset: u32,
    block_length: usize,
}

impl RequestMessage {
    pub fn new(piece_index: u32, block_offset: u32, block_length: usize) -> RequestMessage {
        RequestMessage {
            piece_index: piece_index,
            block_offset: block_offset,
            block_length: block_length,
        }
    }

    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<RequestMessage>> {
        throwaway_input!(parse_request(bytes.as_ref()))
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        try!(message::write_length_id_pair(&mut writer, message::REQUEST_MESSAGE_LEN, Some(message::REQUEST_MESSAGE_ID)));

        try!(writer.write_u32::<BigEndian>(self.piece_index));
        try!(writer.write_u32::<BigEndian>(self.block_offset));
        writer.write_u32::<BigEndian>(self.block_length as u32)
    }

    pub fn piece_index(&self) -> u32 {
        self.piece_index
    }

    pub fn block_offset(&self) -> u32 {
        self.block_offset
    }

    pub fn block_length(&self) -> usize {
        self.block_length
    }
}

fn parse_request(bytes: &[u8]) -> IResult<&[u8], io::Result<RequestMessage>> {
    map!(bytes,
         tuple!(be_u32, be_u32, be_u32),
         |(index, offset, length)| Ok(RequestMessage::new(index, offset, message::u32_to_usize(length)))
    )
}

// ----------------------------------------------------------------------------//

/// Message for sending a block to a peer.
///
/// This message is shallow, meaning it contains the initial message data,
/// but the actual block should be sent to the peer after sending this message.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PieceMessage {
    piece_index:  u32,
    block_offset: u32,
    block:        Bytes
}

impl PieceMessage {
    pub fn new(piece_index: u32, block_offset: u32, block: Bytes) -> PieceMessage {
        // TODO: Check that users Bytes wont overflow a u32 
        PieceMessage {
            piece_index: piece_index,
            block_offset: block_offset,
            block: block
        }
    }

    pub fn parse_bytes(_input: (), bytes: Bytes, len: u32) -> IResult<(), io::Result<PieceMessage>> {
        throwaway_input!(parse_piece(&bytes, len))
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        let actual_length = (9 + self.block_length()) as u32;
        try!(message::write_length_id_pair(&mut writer, actual_length, Some(message::PIECE_MESSAGE_ID)));

        try!(writer.write_u32::<BigEndian>(self.piece_index));
        try!(writer.write_u32::<BigEndian>(self.block_offset));

        writer.write_all(&self.block[..])
    }

    pub fn piece_index(&self) -> u32 {
        self.piece_index
    }

    pub fn block_offset(&self) -> u32 {
        self.block_offset
    }

    pub fn block_length(&self) -> usize {
        self.block.len()
    }

    pub fn block(&self) -> Bytes {
        self.block.clone()
    }
}

fn parse_piece(bytes: &Bytes, len: u32) -> IResult<&[u8], io::Result<PieceMessage>> {
    do_parse!(bytes.as_ref(),
        piece_index:  be_u32                                                    >>
        block_offset: be_u32                                                    >>
        block_len:    value!(message::u32_to_usize(len))                        >>
        block:        map!(take!(block_len), |_| bytes.slice(8, 8 + block_len)) >>
        (Ok(PieceMessage::new(piece_index, block_offset, block)))
    )
}

// ----------------------------------------------------------------------------//

/// Message for cancelling a `RequestMessage` sent to a peer.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct CancelMessage {
    piece_index: u32,
    block_offset: u32,
    block_length: usize,
}

impl CancelMessage {
    pub fn new(piece_index: u32, block_offset: u32, block_length: usize) -> CancelMessage {
        CancelMessage {
            piece_index: piece_index,
            block_offset: block_offset,
            block_length: block_length,
        }
    }

    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<CancelMessage>> {
        throwaway_input!(parse_cancel(bytes.as_ref()))
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        try!(message::write_length_id_pair(&mut writer, message::CANCEL_MESSAGE_LEN, Some(message::CANCEL_MESSAGE_ID)));

        try!(writer.write_u32::<BigEndian>(self.piece_index));
        try!(writer.write_u32::<BigEndian>(self.block_offset));
        writer.write_u32::<BigEndian>(self.block_length as u32)
    }

    pub fn piece_index(&self) -> u32 {
        self.piece_index
    }

    pub fn block_offset(&self) -> u32 {
        self.block_offset
    }

    pub fn block_length(&self) -> usize {
        self.block_length
    }
}

fn parse_cancel(bytes: &[u8]) -> IResult<&[u8], io::Result<CancelMessage>> {
    map!(bytes,
         tuple!(be_u32, be_u32, be_u32),
         |(index, offset, length)| Ok(CancelMessage::new(index, offset, message::u32_to_usize(length)))
    )
}