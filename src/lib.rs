//! # Rust Bittorrent Library

// #![feature(vec_push_all)]

extern crate rand;
extern crate sha1;

pub mod bencode;
pub mod error;
pub mod torrent;

mod info_hash;
mod util;

pub use self::info_hash::{InfoHash, INFO_HASH_LEN};
