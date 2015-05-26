//! # Rust Bittorrent Library

#![feature(collections)]

extern crate rand;
extern crate sha1;

pub mod bencode;
pub mod error;
pub mod torrent;
mod util;
