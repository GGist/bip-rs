use std::io::{self, Write};

use message::PeerWireProtocolMessage;
use protocol::PeerProtocol;

use nom::IResult;

/// Protocol message for peer wire messages.
pub struct PeerWireProtocol<P> {
    ext_protocol: P
}

impl<P> PeerWireProtocol<P> {
    /// Create a new `PeerWireProtocol` with the given extension protocol.
    pub fn new(ext_protocol: P) -> PeerWireProtocol<P> {
        PeerWireProtocol{ ext_protocol: ext_protocol }
    }
}

impl<P> PeerProtocol for PeerWireProtocol<P> where P: PeerProtocol {
    type ProtocolMessage = PeerWireProtocolMessage<P>;

    fn parse_bytes<'a>(&mut self, bytes: &'a [u8]) -> IResult<&'a [u8], Self::ProtocolMessage> {
        PeerWireProtocolMessage::parse_bytes(bytes, &mut self.ext_protocol)
    }

    fn write_bytes<W>(&mut self, message: &Self::ProtocolMessage, writer: W) -> io::Result<()>
        where W: Write {
        message.write_bytes(writer, &mut self.ext_protocol)
    }

    fn message_size(&mut self, message: &Self::ProtocolMessage) -> usize {
        message.message_size(&mut self.ext_protocol)
    }
}