use std::net::SocketAddr;

use crate::message::protocol::Protocol;
use crate::message::extensions::{Extensions};

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
    /// Create a new `CompleteMessage` over the given socket S.
    pub fn new(prot: Protocol, ext: Extensions, hash: InfoHash, pid: PeerId, addr: SocketAddr, sock: S) -> CompleteMessage<S> {
        CompleteMessage{ prot, ext, hash, pid, addr, sock }
    }

    /// Protocol that this peer is operating over.
    pub fn protocol(&self) -> &Protocol {
        &self.prot
    }

    /// Extensions that both you and the peer support.
    pub fn extensions(&self) -> &Extensions {
        &self.ext
    }

    /// Hash that the peer is interested in.
    pub fn hash(&self) -> &InfoHash {
        &self.hash
    }

    /// Id that the peer has given itself.
    pub fn peer_id(&self) -> &PeerId {
        &self.pid
    }

    /// Address the peer is connected to us on.
    pub fn address(&self) -> &SocketAddr {
        &self.addr
    }

    /// Socket of some type S, that we use to communicate with the peer.
    pub fn socket(&self) -> &S {
        &self.sock
    }

    /// Break the `CompleteMessage` into its parts.
    pub fn into_parts(self) -> (Protocol, Extensions, InfoHash, PeerId, SocketAddr, S) {
        (self.prot, self.ext, self.hash, self.pid, self.addr, self.sock)
    }
}