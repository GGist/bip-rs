//! Generic `PeerProtocol` implementations.

use std::io::{self, Write};

use bytes::Bytes;

pub mod extension;
pub mod null;
pub mod unit;
pub mod wire;

/// Trait for implementing a bittorrent protocol message.
pub trait PeerProtocol {
    /// Type of message the protocol operates with.
    type ProtocolMessage;

    /// Total number of bytes needed to parse a complete message. This is not
    /// in addition to what we were given, this is the total number of bytes, so
    /// if the given bytes has length >= needed, then we can parse it.
    ///
    /// If none is returned, it means we need more bytes to determine the number
    /// of bytes needed. If an error is returned, it means the connection should
    /// be dropped, as probably the message exceeded some maximum length.
    fn bytes_needed(&mut self, bytes: &[u8]) -> io::Result<Option<usize>>;

    /// Parse a `ProtocolMessage` from the given bytes.
    fn parse_bytes(&mut self, bytes: Bytes) -> io::Result<Self::ProtocolMessage>;

    /// Write a `ProtocolMessage` to the given writer.
    fn write_bytes<W>(&mut self, message: &Self::ProtocolMessage, writer: W) -> io::Result<()>
    where
        W: Write;

    /// Retrieve how many bytes the message will occupy on the wire.
    fn message_size(&mut self, message: &Self::ProtocolMessage) -> usize;
}

/// Trait for nested peer protocols to see higher level peer protocol messages.
///
/// This is useful when tracking certain states of a connection that happen at a
/// higher level peer protocol, but which nested peer protocols need to know
/// about atomically (before other messages dependent on that message start
/// coming in, and the nested protocol is expected to handle those).
///
/// Example: We handle `ExtensionMessage`s at the `PeerWireProtocol` layer, but
/// the `ExtensionMessage` contains mappings of id to message type that nested
/// extensions need to know about so they can determine what type of message a
/// given id maps to. We need to pass the `ExtensionMessage` down to them before
/// we start receiving messages with those ids (otherwise we will receive
/// unrecognized messages and kill the connection).
pub trait NestedPeerProtocol<M> {
    /// Notify a nested protocol that we have received the given message.
    fn received_message(&mut self, message: &M);

    /// Notify a nested protocol that we have sent the given message.
    fn sent_message(&mut self, message: &M);
}
