use std::hash::Hash;
use std::hash::Hasher;
use std::net::SocketAddr;

use bip_handshake::Extensions;
use bip_util::bt::{InfoHash, PeerId};

/// Information that uniquely identifies a peer.
/// 
/// Equality oprations DO NOT INCLUDE `Extensions` as we define a
/// unique peer as `(address, peer_id, hash)`, so equality will be
/// based on that tuple.
#[derive(Eq, Debug, Copy, Clone)]
pub struct PeerInfo {
    addr: SocketAddr,
    pid:  PeerId,
    hash: InfoHash,
    ext:  Extensions
}

impl PeerInfo {
    /// Create a new `PeerInfo` object.
    pub fn new(addr: SocketAddr, pid: PeerId, hash: InfoHash, extensions: Extensions) -> PeerInfo {
        PeerInfo{ addr, pid, hash, ext: extensions }
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

    /// Retrieve the extensions supported by this peer.
    pub fn extensions(&self) -> &Extensions {
        &self.ext
    }
}

impl PartialEq for PeerInfo {
    fn eq(&self, other: &PeerInfo) -> bool {
        self.addr.eq(&other.addr) && self.pid.eq(&other.pid) && self.hash.eq(&other.hash)
    }
}

impl Hash for PeerInfo {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        self.addr.hash(state);
        self.pid.hash(state);
        self.hash.hash(state);
    }
}