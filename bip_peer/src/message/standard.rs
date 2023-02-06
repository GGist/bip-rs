use std::io::{self, Write};

use byteorder::{BigEndian, WriteBytesExt};
use bytes::Bytes;
use nom::{be_u32, IResult, Needed};

use crate::message;

/// Message for notifying a peer of a piece that you have.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct HaveMessage {
    piece_index: u32,
}

impl HaveMessage {
    pub fn new(piece_index: u32) -> HaveMessage {
        HaveMessage { piece_index }
    }

    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<HaveMessage>> {
        throwaway_input!(parse_have(bytes.as_ref()))
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
    where
        W: Write,
    {
        message::write_length_id_pair(
            &mut writer,
            message::HAVE_MESSAGE_LEN,
            Some(message::HAVE_MESSAGE_ID),
        )?;

        writer.write_u32::<BigEndian>(self.piece_index)
    }

    pub fn piece_index(&self) -> u32 {
        self.piece_index
    }
}

fn parse_have(bytes: &[u8]) -> IResult<&[u8], io::Result<HaveMessage>> {
    map!(bytes, be_u32, |index| Ok(HaveMessage::new(index)))
}

// ---------------------------------------------------------------------------//

/// Message for notifying a peer of all of the pieces you have.
///
/// This should be sent immediately after the handshake.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct BitFieldMessage {
    bytes: Bytes,
}

impl BitFieldMessage {
    pub fn new(bytes: Bytes) -> BitFieldMessage {
        BitFieldMessage { bytes }
    }

    pub fn parse_bytes(
        _input: (),
        mut bytes: Bytes,
        len: u32,
    ) -> IResult<(), io::Result<BitFieldMessage>> {
        let cast_len = message::u32_to_usize(len);

        if bytes.len() >= cast_len {
            IResult::Done(
                (),
                Ok(BitFieldMessage {
                    bytes: bytes.split_to(cast_len),
                }),
            )
        } else {
            IResult::Incomplete(Needed::Size(cast_len - bytes.len()))
        }
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
    where
        W: Write,
    {
        let actual_length = (1 + self.bytes.len()) as u32;
        message::write_length_id_pair(
            &mut writer,
            actual_length,
            Some(message::BITFIELD_MESSAGE_ID),
        )?;

        writer.write_all(&self.bytes)
    }

    pub fn bitfield(&self) -> &[u8] {
        &self.bytes
    }

    pub fn iter(&self) -> BitFieldIter {
        BitFieldIter::new(self.bytes.clone())
    }
}

/// Iterator for a `BitFieldMessage` to `HaveMessage`s.
pub struct BitFieldIter {
    bytes: Bytes,
    // TODO: Probably not the best type for indexing bits?
    cur_bit: usize,
}

impl BitFieldIter {
    fn new(bytes: Bytes) -> BitFieldIter {
        BitFieldIter { bytes, cur_bit: 0 }
    }
}

impl Iterator for BitFieldIter {
    type Item = HaveMessage;

    fn next(&mut self) -> Option<HaveMessage> {
        let byte_in_bytes = self.cur_bit / 8;
        let bit_in_byte = self.cur_bit % 8;

        let opt_byte = self.bytes.get(byte_in_bytes).copied();
        opt_byte.and_then(|byte| {
            let have_message = HaveMessage::new(self.cur_bit as u32);
            self.cur_bit += 1;

            if (byte << bit_in_byte) >> 7 == 1 {
                Some(have_message)
            } else {
                self.next()
            }
        })
    }
}

// ---------------------------------------------------------------------------//

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
            piece_index,
            block_offset,
            block_length,
        }
    }

    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<RequestMessage>> {
        throwaway_input!(parse_request(bytes.as_ref()))
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
    where
        W: Write,
    {
        message::write_length_id_pair(
            &mut writer,
            message::REQUEST_MESSAGE_LEN,
            Some(message::REQUEST_MESSAGE_ID),
        )?;

        writer.write_u32::<BigEndian>(self.piece_index)?;
        writer.write_u32::<BigEndian>(self.block_offset)?;
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
    map!(bytes, tuple!(be_u32, be_u32, be_u32), |(
        index,
        offset,
        length,
    )| Ok(
        RequestMessage::new(index, offset, message::u32_to_usize(length))
    ))
}

// ---------------------------------------------------------------------------//

/// Message for sending a block to a peer.
///
/// This message is shallow, meaning it contains the initial message data,
/// but the actual block should be sent to the peer after sending this message.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PieceMessage {
    piece_index: u32,
    block_offset: u32,
    block: Bytes,
}

impl PieceMessage {
    pub fn new(piece_index: u32, block_offset: u32, block: Bytes) -> PieceMessage {
        // TODO: Check that users Bytes wont overflow a u32
        PieceMessage {
            piece_index,
            block_offset,
            block,
        }
    }

    pub fn parse_bytes(
        _input: (),
        bytes: Bytes,
        len: u32,
    ) -> IResult<(), io::Result<PieceMessage>> {
        throwaway_input!(parse_piece(&bytes, len))
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
    where
        W: Write,
    {
        let actual_length = (9 + self.block_length()) as u32;
        message::write_length_id_pair(&mut writer, actual_length, Some(message::PIECE_MESSAGE_ID))?;

        writer.write_u32::<BigEndian>(self.piece_index)?;
        writer.write_u32::<BigEndian>(self.block_offset)?;

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
    do_parse!(
        bytes.as_ref(),
        piece_index: be_u32
            >> block_offset: be_u32
            >> block_len: value!(message::u32_to_usize(len - 8))
            >> block: map!(take!(block_len), |_| bytes.slice(8, 8 + block_len))
            >> (Ok(PieceMessage::new(piece_index, block_offset, block)))
    )
}

// ---------------------------------------------------------------------------//

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
            piece_index,
            block_offset,
            block_length,
        }
    }

    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<CancelMessage>> {
        throwaway_input!(parse_cancel(bytes.as_ref()))
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
    where
        W: Write,
    {
        message::write_length_id_pair(
            &mut writer,
            message::CANCEL_MESSAGE_LEN,
            Some(message::CANCEL_MESSAGE_ID),
        )?;

        writer.write_u32::<BigEndian>(self.piece_index)?;
        writer.write_u32::<BigEndian>(self.block_offset)?;
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
    map!(bytes, tuple!(be_u32, be_u32, be_u32), |(
        index,
        offset,
        length,
    )| Ok(
        CancelMessage::new(index, offset, message::u32_to_usize(length))
    ))
}

#[cfg(test)]
mod tests {
    use super::{BitFieldMessage, HaveMessage};

    use bytes::Bytes;

    #[test]
    fn positive_bitfield_iter_empty() {
        let bitfield = BitFieldMessage::new(Bytes::new());

        assert_eq!(0, bitfield.iter().count());
    }

    #[test]
    fn positive_bitfield_iter_no_messages() {
        let mut bytes = Bytes::new();
        bytes.extend_from_slice(&[0x00, 0x00, 0x00]);

        let bitfield = BitFieldMessage::new(bytes);

        assert_eq!(0, bitfield.iter().count());
    }

    #[test]
    fn positive_bitfield_iter_single_message_beginning() {
        let mut bytes = Bytes::new();
        bytes.extend_from_slice(&[0x80, 0x00, 0x00]);

        let bitfield = BitFieldMessage::new(bytes);

        assert_eq!(1, bitfield.iter().count());
        assert_eq!(HaveMessage::new(0), bitfield.iter().next().unwrap());
    }

    #[test]
    fn positive_bitfield_iter_single_message_middle() {
        let mut bytes = Bytes::new();
        bytes.extend_from_slice(&[0x00, 0x01, 0x00]);

        let bitfield = BitFieldMessage::new(bytes);

        assert_eq!(1, bitfield.iter().count());
        assert_eq!(HaveMessage::new(15), bitfield.iter().next().unwrap());
    }

    #[test]
    fn positive_bitfield_iter_single_message_ending() {
        let mut bytes = Bytes::new();
        bytes.extend_from_slice(&[0x00, 0x00, 0x01]);

        let bitfield = BitFieldMessage::new(bytes);

        assert_eq!(1, bitfield.iter().count());
        assert_eq!(HaveMessage::new(23), bitfield.iter().next().unwrap());
    }

    #[test]
    fn positive_bitfield_iter_multiple_messages() {
        let mut bytes = Bytes::new();
        bytes.extend_from_slice(&[0xAF, 0x00, 0xC1]);

        let bitfield = BitFieldMessage::new(bytes);
        let messages: Vec<HaveMessage> = bitfield.iter().collect();

        assert_eq!(9, messages.len());
        assert_eq!(
            vec![
                HaveMessage::new(0),
                HaveMessage::new(2),
                HaveMessage::new(4),
                HaveMessage::new(5),
                HaveMessage::new(6),
                HaveMessage::new(7),
                HaveMessage::new(16),
                HaveMessage::new(17),
                HaveMessage::new(23)
            ],
            messages
        );
    }
}
