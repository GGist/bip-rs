use bytes::Bytes;

/// Protocol message for peer wire messages.
pub struct PeerExtensionProtocol<P> {
    custom_protocol: P
}

impl<P> PeerExtensionProtocol<P> {
    /// Create a new `PeerExtensionProtocol` with the given (nested) custom extension protocol.
    ///
    /// Notes for `PeerWireProtocol` apply to this custom extension protocol, so refer to that.
    pub fn new(custom_protocol: P) -> PeerExtensionProtocol<P> {
        PeerExtensionProtocol{ custom_protocol: custom_protocol }
    }
}

impl<P> PeerProtocol for PeerExtensionProtocol<P> where P: PeerProtocol {
    type ProtocolMessage = PeerExtensionProtocolMessage<P>;

    fn bytes_needed(&mut self, bytes: &[u8]) -> io::Result<Option<usize>> {
        PeerExtensionProtocolMessage::bytes_needed(bytes, &mut self.custom_protocol)
    }

    fn parse_bytes(&mut self, bytes: Bytes) -> io::Result<Self::ProtocolMessage> {
        PeerExtensionProtocolMessage::parse_bytes(bytes, &mut self.custom_protocol)
    }

    fn write_bytes<W>(&mut self, message: &Self::ProtocolMessage, writer: W) -> io::Result<()>
        where W: Write {
        message.write_bytes(writer, &mut self.custom_protocol)
    }

    fn message_size(&mut self, message: &Self::ProtocolMessage) -> usize {
        message.message_size(&mut self.custom_protocol)
    }
}