use std::io::{self, Write};

use crate::message::NullProtocolMessage;
use crate::protocol::{NestedPeerProtocol, PeerProtocol};

use bytes::Bytes;

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

    fn bytes_needed(&mut self, _bytes: &[u8]) -> io::Result<Option<usize>> {
        Ok(Some(0))
    }

    fn parse_bytes(&mut self, _bytes: Bytes) -> io::Result<Self::ProtocolMessage> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Attempted To Parse Bytes As Null Protocol",
        ))
    }

    fn write_bytes<W>(&mut self, _message: &Self::ProtocolMessage, _writer: W) -> io::Result<()>
    where
        W: Write,
    {
        panic!("bip_peer: NullProtocol::write_bytes Was Called...Wait, How Did You Construct An Instance Of NullProtocolMessage? :)")
    }

    fn message_size(&mut self, _message: &Self::ProtocolMessage) -> usize {
        0
    }
}

impl<M> NestedPeerProtocol<M> for NullProtocol {
    fn received_message(&mut self, _message: &M) {}

    fn sent_message(&mut self, _message: &M) {}
}
