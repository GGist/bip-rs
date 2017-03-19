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
    /// Create a new `InitiateMessage`.
    pub fn new(prot: Protocol, hash: InfoHash, addr: SocketAddr) -> InitiateMessage {
        InitiateMessage{ prot: prot, hash: hash, addr: addr }
    }

    /// Protocol that we want to connect to the peer with.
    pub fn protocol(&self) -> &Protocol {
        &self.prot
    }

    /// Hash that we are interested in from the peer.
    pub fn hash(&self) -> &InfoHash {
        &self.hash
    }

    /// Address that we should connect to for the peer.
    pub fn address(&self) -> &SocketAddr {
        &self.addr
    }

    /// Break the `InitiateMessage` up into its parts.
    pub fn into_parts(self) -> (Protocol, InfoHash, SocketAddr) {
        (self.prot, self.hash, self.addr)
    }
}