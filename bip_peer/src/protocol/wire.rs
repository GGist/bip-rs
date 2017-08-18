use std::io::{self, Write};

use message::{PeerWireProtocolMessage, ExtendedMessage, BitsExtensionMessage};
use protocol::{PeerProtocol, NestedPeerProtocol};

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

impl<P> PeerProtocol for PeerWireProtocol<P> where P: PeerProtocol + NestedPeerProtocol<ExtendedMessage> {
    type ProtocolMessage = PeerWireProtocolMessage<P>;

    fn bytes_needed(&mut self, bytes: &[u8]) -> io::Result<Option<usize>> {
        PeerWireProtocolMessage::bytes_needed(bytes, &mut self.ext_protocol)
    }

    fn parse_bytes(&mut self, bytes: Bytes) -> io::Result<Self::ProtocolMessage> {
        match PeerWireProtocolMessage::parse_bytes(bytes, &mut self.ext_protocol) {
            Ok(PeerWireProtocolMessage::BitsExtension(BitsExtensionMessage::Extended(msg))) => {
                self.ext_protocol.received_message(&msg);

                Ok(PeerWireProtocolMessage::BitsExtension(BitsExtensionMessage::Extended(msg)))
            },
            other                                                                           => other
        }
    }

    fn write_bytes<W>(&mut self, message: &Self::ProtocolMessage, writer: W) -> io::Result<()>
        where W: Write {
        match (message.write_bytes(writer, &mut self.ext_protocol), message) {
            (Ok(()), &PeerWireProtocolMessage::BitsExtension(BitsExtensionMessage::Extended(ref msg))) => {
                self.ext_protocol.sent_message(msg);
                
                Ok(())
            },
            (other, _)                                                                                 => other
        }
    }

    fn message_size(&mut self, message: &Self::ProtocolMessage) -> usize {
        message.message_size(&mut self.ext_protocol)
    }
}