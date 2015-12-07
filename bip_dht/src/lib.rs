#![feature(into_cow, ip_addr, lookup_addr, ip)]
#![allow(unused)]
//! Interact with the bittorrent Distributed Hash Table.

#[macro_use]
extern crate bip_bencode;
extern crate bip_handshake;
extern crate bip_util;

extern crate crc;
#[macro_use]
extern crate log;
extern crate mio;
extern crate rand;
extern crate chrono;

// Mainline DHT extensions supported on behalf of libtorrent:
// - Always send 'nodes' on a get_peers response even if 'values' is present
// - Unrecognized requests which contain either an 'info_hash' or 'target' arguments are interpreted as 'find_node'
// - Client identification will be present in all outgoing messages in the form of the 'v' key TODO
// const CLIENT_IDENTIFICATION: &'static [u8] = &[b'R', b'D', 0, 1];
// * IPv6 is currently NOT supported in this implementation

// TODO: The Vuze dht operates over a protocol that is different than the mainline dht.
// It would be possible to create a dht client that can work over both dhts simultaneously,
// this would require essentially a completely separate routing table of course and so it
// might make sense to make this distinction available to the user and allow them to startup
// two dhts using the different protocols on their own.
//const VUZE_DHT: (&'static str, u16) = ("dht.aelitis.com", 6881);

mod builder;
mod error;
mod message;
mod router;
mod security;
mod routing;
mod token;
mod transaction;
mod worker;

pub use builder::{DhtBuilder, MainlineDht};
pub use error::{DhtError, DhtResult, DhtErrorKind};
pub use router::{Router};