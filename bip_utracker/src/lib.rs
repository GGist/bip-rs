extern crate bip_handshake;
extern crate bip_util;
extern crate umio;
#[macro_use]
extern crate nom;
extern crate byteorder;

// Action ids used in both requests and responses.
const CONNECT_ACTION_ID:       u32 = 0;
const ANNOUNCE_IPV4_ACTION_ID: u32 = 1;
const SCRAPE_ACTION_ID:        u32 = 2;
const ANNOUNCE_IPV6_ACTION_ID: u32 = 4;

pub mod transaction;

pub mod announce;
pub mod contact;
pub mod error;
pub mod option;
pub mod request;
pub mod response;
pub mod scrape;

pub use bip_handshake::{Handshaker};