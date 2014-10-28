//! This crate provides functionality required to go from simply having a torrent
//! file, to reading that file, to contacting the appropriate trackers, to setting 
//! up port forwards if necessary, and finally, participating in the piece trading 
//! phase and verification of the complete file.
//! 
//! Documentation is still in progress and is very segmented. Some of the documentation
//! is near completion (ex: upnp module) and some of it is non existent (ex: bencode module).

#![feature(phase)]
#![feature(slicing_syntax)]

#[phase(plugin)]

extern crate regex_macros;
extern crate regex;

pub mod bencode;
pub mod torrent;
pub mod tracker;
pub mod tracker_udp;
pub mod upnp;
pub mod util;