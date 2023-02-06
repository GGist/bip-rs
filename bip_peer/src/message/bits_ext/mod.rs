use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use bip_bencode::{BConvert, BDecodeOpt, BMutAccess, BencodeMut, BencodeRef};
use bip_util::convert;
use byteorder::{BigEndian, WriteBytesExt};
use bytes::Bytes;
use nom::{be_u16, be_u32, be_u8, IResult, Needed};

use crate::message;
use crate::message::bencode;

const PORT_MESSAGE_LEN: u32 = 3;
const BASE_EXTENDED_MESSAGE_LEN: u32 = 6;

const PORT_MESSAGE_ID: u8 = 9;
pub const EXTENDED_MESSAGE_ID: u8 = 20;

const EXTENDED_MESSAGE_HANDSHAKE_ID: u8 = 0;

mod handshake;
mod port;

pub use self::handshake::{ExtendedMessage, ExtendedMessageBuilder, ExtendedType};
pub use self::port::PortMessage;

/// Enumeration of messages for `PeerWireProtocolMessage`, activated via
/// `Extensions` bits.
///
/// Sent after the handshake if the corresponding extension bit is set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BitsExtensionMessage {
    /// Messsage for determining the port a peer's DHT is listening on.
    Port(PortMessage),
    /// Message for sending a peer the map of extensions we support.
    Extended(ExtendedMessage),
}

impl BitsExtensionMessage {
    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<BitsExtensionMessage>> {
        parse_extension(bytes)
    }

    pub fn write_bytes<W>(&self, writer: W) -> io::Result<()>
    where
        W: Write,
    {
        match self {
            &BitsExtensionMessage::Port(msg) => msg.write_bytes(writer),
            &BitsExtensionMessage::Extended(ref msg) => msg.write_bytes(writer),
        }
    }

    pub fn message_size(&self) -> usize {
        match self {
            &BitsExtensionMessage::Port(_) => PORT_MESSAGE_LEN as usize,
            &BitsExtensionMessage::Extended(ref msg) => {
                BASE_EXTENDED_MESSAGE_LEN as usize + msg.bencode_size()
            }
        }
    }
}

fn parse_extension(mut bytes: Bytes) -> IResult<(), io::Result<BitsExtensionMessage>> {
    let header_bytes = bytes.clone();

    alt!(
        (),
        ignore_input!(
            switch!(header_bytes.as_ref(), throwaway_input!(tuple!(be_u32, be_u8)),
                (PORT_MESSAGE_LEN, PORT_MESSAGE_ID) => map!(
                    call!(PortMessage::parse_bytes, bytes.split_off(message::HEADER_LEN)),
                    |res_port| res_port.map(BitsExtensionMessage::Port)
                )
            )
        ) | ignore_input!(
            switch!(header_bytes.as_ref(), throwaway_input!(tuple!(be_u32, be_u8, be_u8)),
                (message_len, EXTENDED_MESSAGE_ID, EXTENDED_MESSAGE_HANDSHAKE_ID) => map!(
                    call!(ExtendedMessage::parse_bytes, bytes.split_off(message::HEADER_LEN + 1), message_len - 2),
                    |res_extended| res_extended.map(BitsExtensionMessage::Extended)
                )
            )
        )
    )
}
