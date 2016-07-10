use std::io::{self, Write};

use byteorder::{WriteBytesExt, BigEndian};
use nom::{IResult, be_u32, be_u8, be_u16};

use message;

const PORT_MESSAGE_LEN: u32 = 3;

const PORT_MESSAGE_ID: u8 = 9;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum ExtensionType {
    Port(PortMessage),
}

impl ExtensionType {
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], ExtensionType> {
        parse_extension(bytes)
    }

    pub fn write_bytes<W>(&self, writer: W) -> io::Result<()>
        where W: Write
    {
        match self {
            &ExtensionType::Port(msg) => msg.write_bytes(writer)
        }
    }
}

fn parse_extension(bytes: &[u8]) -> IResult<&[u8], ExtensionType> {
    switch!(bytes, tuple!(be_u32, be_u8),
        (PORT_MESSAGE_LEN, PORT_MESSAGE_ID) => map!(
            call!(PortMessage::from_bytes), |port| ExtensionType::Port(port)
        )
    )
}

// ----------------------------------------------------------------------------//

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct PortMessage {
    port: u16,
}

impl PortMessage {
    pub fn new(port: u16) -> PortMessage {
        PortMessage { port: port }
    }

    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], PortMessage> {
        parse_port(bytes)
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
