use std::io::{self, Write};

use message::NullProtocolMessage;
use protocol::PeerProtocol;

use nom::{IResult, ErrorKind};

/// Null protocol which will return an error if called.
///
/// This protocol is mainly useful for indicating that you do
/// not want to support any `PeerWireProtocolMessage::ProtExtension`
/// messages.
///
/// Of course, you should make sure that you don't tell peers
/// that you support any extended messages.
pub struct NullProtocol;

impl NullProtocol {
    /// Create a new `NullProtocol`.
    pub fn new() -> NullProtocol {
        NullProtocol
    }
}

impl PeerProtocol for NullProtocol {
    type ProtocolMessage = NullProtocolMessage;

    fn parse_bytes<'a>(&mut self, _bytes: &'a [u8]) -> IResult<&'a [u8], Self::ProtocolMessage> {
        IResult::Error(ErrorKind::Custom(0))
    }

    fn write_bytes<W>(&mut self, _message: &Self::ProtocolMessage, _writer: W) -> io::Result<()>
        where W: Write {
        panic!("bip_peer: NullProtocol::write_bytes Was Called...Wait, How Did You Construct An Instance Of NullProtocolMessage? :)")
    }

    fn message_size(&mut self, _message: &Self::ProtocolMessage) -> usize {
        0
    }
}