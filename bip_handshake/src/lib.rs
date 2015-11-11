#![feature(read_exact)]

//! # DO NOT USE THIS CRATE YET!!!

extern crate bip_util;
extern crate threadpool;

mod bittorrent;
mod handshaker;
mod infohash_map;

pub use bittorrent::{BTHandshaker};
pub use handshaker::{Handshaker};