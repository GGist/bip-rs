extern crate bip_handshake;
extern crate bip_metainfo;
extern crate bip_peer;
extern crate bip_util;
extern crate bip_utracker;
extern crate bit_set;
extern crate bytes;
#[macro_use]
extern crate error_chain;
extern crate futures;
#[macro_use]
extern crate log;
extern crate rand;

#[cfg(test)]
extern crate futures_test;

use bip_metainfo::Metainfo;
use bip_peer::PeerInfo;
use std::time::Duration;

pub mod discovery;
pub mod error;
pub mod revelation;

mod extended;
mod uber;

pub use crate::extended::{ExtendedListener, ExtendedPeerInfo, IExtendedMessage, OExtendedMessage};
pub use crate::uber::{IUberMessage, OUberMessage, UberModule, UberModuleBuilder};

/// Enumeration of control messages most modules will be interested in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlMessage {
    /// Start tracking the given torrent.
    AddTorrent(Metainfo),
    /// Stop tracking the given torrent.
    RemoveTorrent(Metainfo),
    /// Connected to the given peer.
    ///
    /// This message can be sent multiple times, which
    /// is useful if extended peer information changes.
    PeerConnected(PeerInfo),
    /// Disconnected from the given peer.
    PeerDisconnected(PeerInfo),
    /// A span of time has passed.
    ///
    /// This message is vital for certain modules
    /// to function correctly. Subsequent durations
    /// should not be spread too far apart.
    Tick(Duration),
}
