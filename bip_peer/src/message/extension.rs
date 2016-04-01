use nom::{IResult, be_u32, be_u8, be_u16};

const PORT_MESSAGE_LEN: u32 = 3;

const PORT_MESSAGE_ID: u8 = 9;

pub enum ExtensionType {
    Port(PortMessage)
}

impl ExtensionType {
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], ExtensionType> {
        parse_extension(bytes)
    }
}

fn parse_extension(bytes: &[u8]) -> IResult<&[u8], ExtensionType> {
    switch!(bytes, tuple!(be_u32, be_u8),
        (PORT_MESSAGE_LEN, PORT_MESSAGE_ID) => map!(
            call!(PortMessage::from_bytes), |port| ExtensionType::Port(port)
        )
    )
}

//----------------------------------------------------------------------------//

pub struct PortMessage {
    port: u16
}

impl PortMessage {
    pub fn new(port: u16) -> PortMessage {
        PortMessage{ port: port }
    }
    
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], PortMessage> {
        parse_port(bytes)
    }
}

fn parse_port(bytes: &[u8]) -> IResult<&[u8], PortMessage> {
    map!(bytes, be_u16, |port| PortMessage::new(port) )
}