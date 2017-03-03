use std::net::SocketAddr;

use message::protocol::Protocol;

use bip_util::bt::InfoHash;

/// Message used to initiate a handshake with the `Handshaker`.
pub struct InitiateMessage {
    prot: Protocol,
    hash: InfoHash,
    addr: SocketAddr
}

impl InitiateMessage {
    pub fn new(prot: Protocol, hash: InfoHash, addr: SocketAddr) -> InitiateMessage {
        InitiateMessage{ prot: prot, hash: hash, addr: addr }
    }

    pub fn protocol(&self) -> &Protocol {
        &self.prot
    }

    pub fn hash(&self) -> &InfoHash {
        &self.hash
    }

    pub fn address(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn into_parts(self) -> (Protocol, InfoHash, SocketAddr) {
        (self.prot, self.hash, self.addr)
    }
}