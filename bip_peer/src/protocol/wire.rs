use std::io::{self, Write};

use message::PeerWireProtocolMessage;
use protocol::PeerProtocol;

use bytes::Bytes;

/// Protocol message for peer wire messages.
pub struct PeerWireProtocol<P> {
    ext_protocol: P
}

impl<P> PeerWireProtocol<P> {
    /// Create a new `PeerWireProtocol` with the given extension protocol.
    ///
    /// Important to note that nested protocol should follow the same message length format
    /// as the peer wire protocol. This means it should expect a 4 byte (`u32`) message
    /// length prefix. Nested protocols will NOT have their `bytes_needed` method called.
    pub fn new(ext_protocol: P) -> PeerWireProtocol<P> {
        PeerWireProtocol{ ext_protocol: ext_protocol }
    }
}

impl<P> PeerProtocol for PeerWireProtocol<P> where P: PeerProtocol {
    type ProtocolMessage = PeerWireProtocolMessage<P>;

    fn bytes_needed(&mut self, bytes: &[u8]) -> io::Result<Option<usize>> {
        PeerWireProtocolMessage::bytes_needed(bytes, &mut self.ext_protocol)
    }

    fn parse_bytes(&mut self, bytes: Bytes) -> io::Result<Self::ProtocolMessage> {
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