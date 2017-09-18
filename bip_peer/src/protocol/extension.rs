use std::io::{self, Write};

use bytes::Bytes;

use message::{ExtendedMessage, PeerExtensionProtocolMessage};
use protocol::{PeerProtocol, NestedPeerProtocol};

/// Protocol message for peer wire messages.
pub struct PeerExtensionProtocol<P> {
    our_extended_msg:   Option<ExtendedMessage>,
    their_extended_msg: Option<ExtendedMessage>,
    custom_protocol:    P
}

impl<P> PeerExtensionProtocol<P> {
    /// Create a new `PeerExtensionProtocol` with the given (nested) custom extension protocol.
    ///
    /// Notes for `PeerWireProtocol` apply to this custom extension protocol, so refer to that.
    pub fn new(custom_protocol: P) -> PeerExtensionProtocol<P> {
        PeerExtensionProtocol{ our_extended_msg: None, their_extended_msg: None, custom_protocol: custom_protocol }
    }
}

impl<P> PeerProtocol for PeerExtensionProtocol<P> where P: PeerProtocol {
    type ProtocolMessage = PeerExtensionProtocolMessage<P>;

    fn bytes_needed(&mut self, bytes: &[u8]) -> io::Result<Option<usize>> {
        PeerExtensionProtocolMessage::<P>::bytes_needed(bytes)
    }

    fn parse_bytes(&mut self, bytes: Bytes) -> io::Result<Self::ProtocolMessage> {
        match self.their_extended_msg {
            Some(ref extended_msg) => PeerExtensionProtocolMessage::parse_bytes(bytes, extended_msg, &mut self.custom_protocol),
            None                   => Err(io::Error::new(io::ErrorKind::Other, "Extension Message Received From Peer Before Extended Message..."))
        }
    }

    fn write_bytes<W>(&mut self, message: &Self::ProtocolMessage, writer: W) -> io::Result<()>
        where W: Write {
        match self.our_extended_msg {
            Some(ref extended_msg) => PeerExtensionProtocolMessage::write_bytes(message, writer, extended_msg, &mut self.custom_protocol),
            None                   => Err(io::Error::new(io::ErrorKind::Other, "Extension Message Sent From Us Before Extended Message..."))
        }
    }

    fn message_size(&mut self, message: &Self::ProtocolMessage) -> usize {
        message.message_size(&mut self.custom_protocol)
    }
}

impl<P> NestedPeerProtocol<ExtendedMessage> for PeerExtensionProtocol<P> where P: NestedPeerProtocol<ExtendedMessage> {
    fn received_message(&mut self, message: &ExtendedMessage) {
        self.custom_protocol.received_message(message);

        self.their_extended_msg = Some(message.clone());
    }

    fn sent_message(&mut self, message: &ExtendedMessage) {
        self.custom_protocol.sent_message(message);

        self.our_extended_msg = Some(message.clone());
    }
}