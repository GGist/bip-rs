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
}