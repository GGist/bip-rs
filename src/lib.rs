//! # Rust Bittorrent Library
//! This library is a dependency-free implementation of the bittorrent protocol
//! and related extensions. Basic primitives are provided to allow you to connect
//! to a tracker and communicate with other peers within a swarm.
//!
//! The interface for the library primitives allow you to build applications that
//! leverage the protocol in a variety of ways that are not just limited to 
//! bittorrent clients.
//! # Examples
//! Lets say you just want to scrape a tracker to gather statistics on a variety
//! of torrent files. This is similar to what popular torrent hosting sites use
//! to provide statistics to users about how many seeders and leechers a particular
//! torrent file has at any moment in real time. You can get this information
//! with the following snippet:
//!
//! ```no_compile
//! extern crate bittorrent;
//! extern crate crypto;
//! 
//! use std::io::fs::{File};
//! use crypto::sha1::{Sha1};
//! use crypto::digest::{Digest};
//! use bittorrent::bencode::{Bencode};
//! use bittorrent::tracker::udp::{UdpTracker};
//! use bittorrent::tracker::{Tracker};
//! use bittorrent::torrent::{Torrent};
//! 
//! fn main() {
//!     let mut torr_file = File::open(&Path::new("tests/data/test.torrent"));
//!     let torr_bytes = torr_file.read_to_end().unwrap();
//!     let ben_val = Bencode::new(&torr_bytes[]).unwrap();
//!     
//!     let info_dict = ben_val.dict().unwrap().get("info")
//!         .unwrap().encoded();
//!     let torrent = Torrent::new(&ben_val).unwrap();
//!     let (_, name) = torrent.file_type();
//!     
//!     let mut sha = Sha1::new();
//!     let mut result = [0u8; 20];
//!     sha.input(&info_dict[]);
//!     sha.result(result.as_mut_slice());
//!     
//!     let mut tracker = UdpTracker::new(torrent.announce(), &result)
//!         .unwrap();
//!     let scrape_response = tracker.scrape().unwrap();
//!     
//!     println!("Torrent Name:{} Leechers:{} Seeders:{} Total Downloads:{}", 
//!         name, 
//!         scrape_response.leechers, 
//!         scrape_response.seeders, 
//!         scrape_response.downloads
//!     );
//! }
//! ```
//!
//! This is just a simple example, and if you are going to implement a full bittorrent
//! client using this library, you will have to do a bit more work. For example,
//! you are going to want to implement an efficient choking algorithm, some peer 
//! selection heuristics, as well as an end-game algorithm. All of these are required 
//! in order to be competitive with commercial client implementations.

#![unstable]

#![allow(unstable)]
#![feature(unboxed_closures)]
#![feature(plugin)]
#[plugin] #[no_link]

extern crate regex_macros;
extern crate regex;

pub mod bencode;
pub mod error;
pub mod peer;
pub mod torrent;
pub mod tracker;
pub mod upnp;
pub mod util;