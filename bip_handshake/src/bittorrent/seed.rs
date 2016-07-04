use std::net::SocketAddr;

use bip_util::bt::{InfoHash, PeerId};

pub struct InitiateSeed(PartialBTSeed, Option<PeerId>);

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

pub struct CompleteSeed(EmptyBTSeed);

impl CompleteSeed {
    pub fn new(addr: SocketAddr) -> CompleteSeed {
        CompleteSeed(EmptyBTSeed::new(addr))
    }
}

// ----------------------------------------------------------------------------//

pub struct EmptyBTSeed {
    addr: SocketAddr
}

impl EmptyBTSeed {
    fn new(addr: SocketAddr) -> EmptyBTSeed {
        EmptyBTSeed{ addr: addr }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn found(self, hash: InfoHash) -> PartialBTSeed {
        PartialBTSeed { addr: self.addr, hash: hash }
    }
}

pub struct PartialBTSeed {
    addr: SocketAddr,
    hash: InfoHash
}

impl PartialBTSeed {
        pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn found(self, pid: PeerId) -> BTSeed {
        BTSeed{ addr: self.addr, hash: self.hash, pid: pid }
    }
}

pub struct BTSeed {
    addr: SocketAddr,
    hash: InfoHash,
    pid: PeerId
}