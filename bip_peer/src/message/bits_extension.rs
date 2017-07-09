use std::io::{self, Write};

use bytes::Bytes;
use byteorder::{WriteBytesExt, BigEndian};
use nom::{IResult, be_u32, be_u8, be_u16};

use message;

const PORT_MESSAGE_LEN: u32 = 3;

const PORT_MESSAGE_ID: u8 = 9;

/// Enumeration of messages for `PeerWireProtocolMessage`, activated via `Extensions` bits.
///
/// Sent after the handshake if the corresponding extension bit is set.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum BitsExtensionMessage {
    Port(PortMessage),
}

impl BitsExtensionMessage {
    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<BitsExtensionMessage>> {
        parse_extension(bytes)
    }

    pub fn write_bytes<W>(&self, writer: W) -> io::Result<()>
        where W: Write
    {
        match self {
            &BitsExtensionMessage::Port(msg) => msg.write_bytes(writer),
        }
    }

    pub fn message_size(&self) -> usize {
        match self {
            &BitsExtensionMessage::Port(msg) => PORT_MESSAGE_LEN as usize
        }
    }
}

fn parse_extension(mut bytes: Bytes) -> IResult<(), io::Result<BitsExtensionMessage>> {
    let header_bytes = bytes.clone();

    switch!(header_bytes.as_ref(), throwaway_input!(tuple!(be_u32, be_u8)),
        (PORT_MESSAGE_LEN, PORT_MESSAGE_ID) => map!(
            call!(PortMessage::parse_bytes, bytes.split_off(message::HEADER_LEN)),
            |res_port| res_port.map(|port| BitsExtensionMessage::Port(port))
        )
    )
}

// ----------------------------------------------------------------------------//

/// Message for notifying a peer of our DHT port.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct PortMessage {
    port: u16,
}

impl PortMessage {
    pub fn new(port: u16) -> PortMessage {
        PortMessage { port: port }
    }

    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<PortMessage>> {
        match parse_port(bytes.as_ref()) {
            IResult::Done(_, result)  => IResult::Done((), Ok(result)),
            IResult::Error(err)       => IResult::Error(err),
            IResult::Incomplete(need) => IResult::Incomplete(need)
        }
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        try!(message::write_length_id_pair(&mut writer, PORT_MESSAGE_LEN, Some(PORT_MESSAGE_ID)));

        writer.write_u16::<BigEndian>(self.port)
    }
}

fn parse_port(bytes: &[u8]) -> IResult<&[u8], PortMessage> {
    map!(bytes, be_u16, |port| PortMessage::new(port))
}
