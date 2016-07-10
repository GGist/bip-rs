#![allow(unused)]

use std::io::{self, Write};

use byteorder::{WriteBytesExt, BigEndian};
use nom::{IResult, be_u32, be_u8};

use message::extension::ExtensionType;
use message::standard::{HaveMessage, BitFieldMessage, RequestMessage, PieceMessage, CancelMessage};

pub const KEEP_ALIVE_MESSAGE_LEN: u32 = 0;
pub const CHOKE_MESSAGE_LEN: u32 = 1;
pub const UNCHOKE_MESSAGE_LEN: u32 = 1;
pub const INTERESTED_MESSAGE_LEN: u32 = 1;
pub const UNINTERESTED_MESSAGE_LEN: u32 = 1;
pub const HAVE_MESSAGE_LEN: u32 = 5;
pub const REQUEST_MESSAGE_LEN: u32 = 13;
pub const CANCEL_MESSAGE_LEN: u32 = 13;

pub const CHOKE_MESSAGE_ID: u8 = 0;
pub const UNCHOKE_MESSAGE_ID: u8 = 1;
pub const INTERESTED_MESSAGE_ID: u8 = 2;
pub const UNINTERESTED_MESSAGE_ID: u8 = 3;
pub const HAVE_MESSAGE_ID: u8 = 4;
pub const BITFIELD_MESSAGE_ID: u8 = 5;
pub const REQUEST_MESSAGE_ID: u8 = 6;
pub const PIECE_MESSAGE_ID: u8 = 7;
pub const CANCEL_MESSAGE_ID: u8 = 8;

pub const MESSAGE_LENGTH_LEN_BYTES: usize = 4;

pub mod extension;
pub mod standard;

/// Enumeration of all message types. These types are shallow so they do not include
/// variable length payload data; that data will have to be read in afterward.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum MessageType {
    KeepAlive,
    Choke,
    UnChoke,
    Interested,
    UnInterested,
    Have(HaveMessage),
    BitField(BitFieldMessage),
    Request(RequestMessage),
    Piece(PieceMessage),
    Cancel(CancelMessage),
    Extension(ExtensionType),
}

impl MessageType {
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], MessageType> {
        parse_message(bytes)
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        match self {
            &MessageType::KeepAlive => write_length_id_pair(writer, KEEP_ALIVE_MESSAGE_LEN, None),
            &MessageType::Choke => write_length_id_pair(writer, CHOKE_MESSAGE_LEN, Some(CHOKE_MESSAGE_ID)),
            &MessageType::UnChoke => write_length_id_pair(writer, UNCHOKE_MESSAGE_LEN, Some(UNCHOKE_MESSAGE_ID)),
            &MessageType::Interested => write_length_id_pair(writer, INTERESTED_MESSAGE_LEN, Some(INTERESTED_MESSAGE_ID)),
            &MessageType::UnInterested => write_length_id_pair(writer, UNINTERESTED_MESSAGE_LEN, Some(UNINTERESTED_MESSAGE_ID)),
            &MessageType::Have(ref msg) => msg.write_bytes(writer),
            &MessageType::BitField(ref msg) => msg.write_bytes(writer),
            &MessageType::Request(ref msg) => msg.write_bytes(writer),
            &MessageType::Piece(ref msg) => msg.write_bytes(writer),
            &MessageType::Cancel(ref msg) => msg.write_bytes(writer),
            &MessageType::Extension(ref ext) => ext.write_bytes(writer)
        }
    }
}

/// Write a length and optional id out to the given writer.
pub fn write_length_id_pair<W>(mut writer: W, length: u32, opt_id: Option<u8>) -> io::Result<()>
    where W: Write {
    try!(writer.write_u32::<BigEndian>(length));

    if let Some(id) = opt_id {
        writer.write_u8(id)
    } else {
        Ok(())
    }
}

/// Parse the length portion of a message.
///
/// Panics if parsing failed for any reason.
pub fn parse_message_length(bytes: &[u8]) -> usize {
    if let IResult::Done(_, len) = be_u32(bytes) {
        u32_to_usize(len)
    } else {
        panic!("bip_peer: Message Length Was Less Than 4 Bytes")
    }
}

/// Called when a conversion from a u32 to a usize is necessary
/// for the program to proceed in a valid state.
///
/// If the conversion is not valid, a panic will occur.
pub fn u32_to_usize(value: u32) -> usize {
    if value as usize as u32 != value {
        panic!("bip_peer: Cannot Convert u32 To usize, usize Is Less Than 32-Bits")
    }

    value as usize
}

// Since these messages may come over a stream oriented protocol, if a message is incomplete
// the number of bytes needed will be returned. However, that number of bytes is on a per parser
// basis. If possible, we should return the number of bytes needed for the rest of the WHOLE message.
// This allows clients to only re invoke the parser when it knows it has enough of the data.
fn parse_message(bytes: &[u8]) -> IResult<&[u8], MessageType> {
    // Attempt to parse a built in message type, otherwise, see if it is an extension type.
    alt!(bytes,
         switch!(tuple!(be_u32, opt!(be_u8)),
            (KEEP_ALIVE_MESSAGE_LEN, None) => value!(
                MessageType::KeepAlive
            ) |
            (CHOKE_MESSAGE_LEN, Some(CHOKE_MESSAGE_ID)) => value!(
                MessageType::Choke
            ) |
            (UNCHOKE_MESSAGE_LEN, Some(UNCHOKE_MESSAGE_ID)) => value!(
                MessageType::UnChoke
            ) |
            (INTERESTED_MESSAGE_LEN, Some(INTERESTED_MESSAGE_ID)) => value!(
                MessageType::Interested
            ) |
            (UNINTERESTED_MESSAGE_LEN, Some(UNINTERESTED_MESSAGE_ID)) => value!(
                MessageType::UnInterested
            ) |
            (HAVE_MESSAGE_LEN, Some(HAVE_MESSAGE_ID)) => map!(
                call!(HaveMessage::from_bytes),
                |have| MessageType::Have(have)
            ) |
            (message_len, Some(BITFIELD_MESSAGE_ID)) => map!(
                call!(BitFieldMessage::from_bytes, message_len - 1),
                |bitfield| MessageType::BitField(bitfield)
            ) |
            (REQUEST_MESSAGE_LEN, Some(REQUEST_MESSAGE_ID)) => map!(
                call!(RequestMessage::from_bytes),
                |request| MessageType::Request(request)
            ) |
            (message_len, Some(PIECE_MESSAGE_ID)) => map!(
                call!(PieceMessage::from_bytes, message_len - 9),
                |piece| MessageType::Piece(piece)
            ) |
            (CANCEL_MESSAGE_LEN, Some(CANCEL_MESSAGE_ID)) => map!(
                call!(CancelMessage::from_bytes),
                |cancel| MessageType::Cancel(cancel)
            )
         ) | map!(call!(ExtensionType::from_bytes), |ext_type| MessageType::Extension(ext_type)))
}
