use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::ops::{Deref, DerefMut};

use bip_util::bt::{PeerId, InfoHash};

// Used because we don't want to expose the internals of our BTContext<C> to
// peer protocol implementations that will (should) only be interested in derefing
// and accessing their context C.

pub fn peer_context_new<C>(protocol: &'static str, pid: PeerId, interest: Arc<RwLock<HashSet<InfoHash>>>, c_context: C) -> BTContext<C> {
    BTContext {
        protocol: protocol,
        pid: pid,
        interest: interest,
        c_context: c_context,
    }
}

pub fn peer_context_interest<C>(context: &BTContext<C>, hash: &InfoHash) -> bool {
    context.interest.read().expect("bip_handshake: Failed To Lock InfoHash Interest Map").contains(hash)
}

pub fn peer_context_pid<C>(context: &BTContext<C>) -> PeerId {
    context.pid
}

pub fn peer_context_protocol<C>(context: &BTContext<C>) -> &'static str {
    context.protocol
}

// ----------------------------------------------------------------------------//

/// Bittorrent context for a `PeerProtocol` state machine.
pub struct BTContext<C> {
    protocol: &'static str,
    pid: PeerId,
    interest: Arc<RwLock<HashSet<InfoHash>>>,
    c_context: C,
}

impl<C> Deref for BTContext<C> {
    type Target = C;

    fn deref(&self) -> &C {
        &self.c_context
    }
}

impl<C> DerefMut for BTContext<C> {
    fn deref_mut(&mut self) -> &mut C {
        &mut self.c_context
    }
}