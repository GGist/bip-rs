use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::default::Default;

use bip_util::bt::{self, InfoHash, PeerId};

#[derive(Copy, Clone)]
pub struct InitiateSeed(pub PartialBTSeed, pub Option<PeerId>);

impl InitiateSeed {
    pub fn new(addr: SocketAddr, hash: InfoHash) -> InitiateSeed {
        InitiateSeed(EmptyBTSeed::new(addr).found(hash), None)
    }

    pub fn expect_pid(addr: SocketAddr, hash: InfoHash, pid: PeerId) -> InitiateSeed {
        InitiateSeed(EmptyBTSeed::new(addr).found(hash), Some(pid))
    }

    pub fn addr(&self) -> SocketAddr {
        self.0.addr()
    }
}

#[derive(Copy, Clone)]
pub struct CompleteSeed(pub EmptyBTSeed);

impl CompleteSeed {
    pub fn new(addr: SocketAddr) -> CompleteSeed {
        CompleteSeed(EmptyBTSeed::new(addr))
    }
}

// ----------------------------------------------------------------------------//

#[derive(Copy, Clone)]
pub struct EmptyBTSeed {
    addr: SocketAddr,
}

impl EmptyBTSeed {
    fn new(addr: SocketAddr) -> EmptyBTSeed {
        EmptyBTSeed { addr: addr }
    }

    pub fn found(self, hash: InfoHash) -> PartialBTSeed {
        PartialBTSeed {
            addr: self.addr,
            hash: hash,
        }
    }
}

#[derive(Copy, Clone)]
pub struct PartialBTSeed {
    addr: SocketAddr,
    hash: InfoHash,
}

impl PartialBTSeed {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn hash(&self) -> InfoHash {
        self.hash
    }

    pub fn found(self, pid: PeerId) -> BTSeed {
        BTSeed {
            addr: self.addr,
            hash: self.hash,
            pid: pid,
        }
    }
}

/// Bittorrent seed for a `PeerProtocol` state machine.
#[derive(Copy, Clone)]
pub struct BTSeed {
    addr: SocketAddr,
    hash: InfoHash,
    pid: PeerId,
}

impl BTSeed {
    /// Address of the remote peer.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// InfoHash the remote peer is interested in.
    pub fn hash(&self) -> InfoHash {
        self.hash
    }

    /// PeerId of the remote peer.
    pub fn pid(&self) -> PeerId {
        self.pid
    }
}

impl Default for BTSeed {
    fn default() -> BTSeed {
        BTSeed {
            addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)),
            hash: [0u8; bt::INFO_HASH_LEN].into(),
            pid: [0u8; bt::PEER_ID_LEN].into(),
        }
    }
}
