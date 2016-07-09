use std::net::SocketAddr;

use bip_util::bt::{InfoHash, PeerId};

/// Trait for forwarding peer contact information and metadata.
pub trait Handshaker: Send {
    /// Type that metadata will be passed back to the client as.
    type MetadataEnvelope: Send;

    /// PeerId exposed to peer discovery services.
    fn id(&self) -> PeerId;

    /// Port exposed to peer discovery services.
    ///
    /// It is important that this is the external port that the peer will be sending data
    /// to. This is relevant if the client employs nat traversal via upnp or other means.
    fn port(&self) -> u16;

    /// Connect to the given address with the InfoHash and expecting the PeerId.
    fn connect(&mut self, expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr);

    /// Send the given Metadata back to the client.
    fn metadata(&mut self, data: Self::MetadataEnvelope);
}
