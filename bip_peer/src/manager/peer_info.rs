use std::net::SocketAddr;

use bip_util::bt::{InfoHash, PeerId};

/// Information that uniquely identifies a peer.
#[derive(PartialEq, Eq, Hash, Debug, Copy, Clone)]
pub struct PeerInfo {
    addr: SocketAddr,
    pid:  PeerId,
    hash: InfoHash
}

impl PeerInfo {
    /// Create a new `PeerInfo` object.
    pub fn new(addr: SocketAddr, pid: PeerId, hash: InfoHash) -> PeerInfo {
        PeerInfo{ addr: addr, pid: pid, hash: hash }
    }

    /// Retrieve the peer address.
    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    /// Retrieve the peer id.
    pub fn peer_id(&self) -> &PeerId {
        &self.pid
    }
    
    /// Retrieve the peer info hash.
    pub fn hash(&self) -> &InfoHash {
        &self.hash
    }
}