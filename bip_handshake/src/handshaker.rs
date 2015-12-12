use std::net::{SocketAddr};

use bip_util::bt::{InfoHash, PeerId};

/// Trait for providing a handshaker object with connection information.
pub trait Handshaker: Send {
    /// Type of stream used to receive connections from.
    type Stream;

    /// Unique PeerId used to identify ourselves to other peers.
    fn id(&self) -> PeerId;

    /// Advertise port that is being listened on by the handshaker.
    ///
    /// It is important that this is the external port that the peer will be sending data
    /// to. This is relevant if the client employs nat traversal via upnp or other means.
    fn port(&self) -> u16;

    /// Initiates a handshake with the given socket address for the given InfoHash.
    fn connect(&mut self, expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr);
    
    /// Adds a filter that is applied to handshakes before they are initiated or completed.
    fn filter<F>(&mut self, process: Box<F>) where F: Fn(SocketAddr) -> bool + Send;
    
    /// Stream that connections for the specified hash are sent to after they are successful.
    ///
    /// Connections MAY be dropped if all streams for a given hash are not active.
    fn stream(&self, hash: InfoHash) -> Self::Stream;
}