//! # DO NOT USE THIS CRATE YET!!!

extern crate bip_util;
extern crate threadpool;

mod bittorrent;
mod handshaker;
mod infohash_map;

pub use bittorrent::{PeerInfo, BTHandshaker};
pub use handshaker::{Handshaker};

pub use bip_util::bt::{PeerId, InfoHash};