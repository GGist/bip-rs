use std::net::{SocketAddr};

use bip_util::{InfoHash, PeerId};

/// Trait for providing a handshaker object with connection information.
pub trait Handshaker: Send {
    /// Type of stream used to receive connections from.
    type Stream;

    /// Unique peer id used to identify ourselves to other peers.
    fn id(&self) -> PeerId;

    /// Advertise port that is being listened on by the handshaker.
    ///
    /// It is important that this is the external port that the peer will be sending data
    /// to. This is relevant if the client employs nat traversal via upnp or other means.
    fn port(&self) -> u16;

    /// Initiates a handshake with the given socket address.
    fn connect(&mut self, expected: PeerId, hash: InfoHash, addr: SocketAddr);
    
    /// Adds a filter that is applied to handshakes before they are initiated or completed.
    fn filter<F>(&mut self, process: Box<F>) where F: Fn(SocketAddr) -> bool + Send;
    
    /// Stream that connections for the specified hash are sent to after they are successful.
    ///
    /// Connections MAY be dropped if all streams for a given hash are destroyed.
    fn stream(&self, hash: InfoHash) -> Self::Stream;
}