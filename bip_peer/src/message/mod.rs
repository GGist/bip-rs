#![allow(unused)]

use nom::{IResult, be_u32, be_u8};

use message::extension::{ExtensionType};
use message::standard::{HaveMessage, BitFieldMessage, RequestMessage,
    PieceMessage, CancelMessage};

const KEEP_ALIVE_MESSAGE_LEN:   u32 = 0;
const CHOKE_MESSAGE_LEN:        u32 = 1;
const UNCHOKE_MESSAGE_LEN:      u32 = 1;
const INTERESTED_MESSAGE_LEN:   u32 = 1;
const UNINTERESTED_MESSAGE_LEN: u32 = 1;
const HAVE_MESSAGE_LEN:         u32 = 5;
const REQUEST_MESSAGE_LEN:      u32 = 13;
const CANCEL_MESSAGE_LEN:       u32 = 13;

const CHOKE_MESSAGE_ID:        u8 = 0;
const UNCHOKE_MESSAGE_ID:      u8 = 1;
const INTERESTED_MESSAGE_ID:   u8 = 2;
const UNINTERESTED_MESSAGE_ID: u8 = 3;
const HAVE_MESSAGE_ID:         u8 = 4;
const BITFIELD_MESSAGE_ID:     u8 = 5;
const REQUEST_MESSAGE_ID:      u8 = 6;
const PIECE_MESSAGE_ID:        u8 = 7;
const CANCEL_MESSAGE_ID:       u8 = 8;

mod extension;
mod standard;

pub enum MessageType<'a> {
    KeepAlive,
    Choke,
    UnChoke,
    Interested,
    UnInterested,
    Have(HaveMessage),
    BitField(BitFieldMessage<'a>),
    Request(RequestMessage),
    Piece(PieceMessage<'a>),
    Cancel(CancelMessage),
    Extension(ExtensionType)
}

impl<'a> MessageType<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> IResult<&'a [u8], MessageType<'a>> {
        parse_message(bytes)
    }
}

// Since these messages may come over a stream oriented protocol, if a message is incomplete
// the number of bytes needed will be returned. However, that number of bytes is on a per parser
// basis. If possible, we should return the number of bytes needed for the rest of the WHOLE message.
// This allows clients to only re invoke the parser when it knows it has enough of the data.
fn parse_message<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], MessageType<'a>> {
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
        ) |
        map!(call!(ExtensionType::from_bytes), |ext_type| {
            MessageType::Extension(ext_type)
        })
    )
}