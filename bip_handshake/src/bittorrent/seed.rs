use std::net::SocketAddr;

use bip_util::bt::{InfoHash, PeerId};

pub struct InitiateSeed {
    pid: Option<PeerId>,
    addr: SocketAddr,
    hash: InfoHash,
}

impl InitiateSeed {
    pub fn new(addr: SocketAddr, hash: InfoHash) -> InitiateSeed {
        InitiateSeed {
            pid: None,
            addr: addr,
            hash: hash,
        }
    }

    pub fn with_pid(pid: PeerId, addr: SocketAddr, hash: InfoHash) -> InitiateSeed {
        InitiateSeed {
            pid: Some(pid),
            addr: addr,
            hash: hash,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

// ----------------------------------------------------------------------------//

pub struct CompleteSeed {
    addr: SocketAddr,
}

impl CompleteSeed {
    pub fn new(addr: SocketAddr) -> CompleteSeed {
        CompleteSeed { addr: addr }
    }
}

// ----------------------------------------------------------------------------//

pub struct BTSeed {
    pid: PeerId,
    addr: SocketAddr,
    hash: InfoHash,
}
