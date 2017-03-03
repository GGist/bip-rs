use std::net::SocketAddr;

use message::protocol::Protocol;
use message::extensions::{Extensions};

use bip_util::bt::{InfoHash, PeerId};

/// Message containing completed handshaking information.
pub struct CompleteMessage<S> {
    prot: Protocol,
    ext:  Extensions,
    hash: InfoHash,
    pid:  PeerId,
    addr: SocketAddr,
    sock: S
}

impl<S> CompleteMessage<S> {
    pub fn new(prot: Protocol, ext: Extensions, hash: InfoHash, pid: PeerId, addr: SocketAddr, sock: S) -> CompleteMessage<S> {
        CompleteMessage{ prot: prot, ext: ext, hash: hash, pid: pid, addr: addr, sock: sock }
    }

    pub fn protocol(&self) -> &Protocol {
        &self.prot
    }

    pub fn extensions(&self) -> &Extensions {
        &self.ext
    }

    pub fn hash(&self) -> &InfoHash {
        &self.hash
    }

    pub fn peer_id(&self) -> &PeerId {
        &self.pid
    }

    pub fn address(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn socket(&self) -> &S {
        &self.sock
    }

    pub fn into_parts(self) -> (Protocol, Extensions, InfoHash, PeerId, SocketAddr, S) {
        (self.prot, self.ext, self.hash, self.pid, self.addr, self.sock)
    }
}