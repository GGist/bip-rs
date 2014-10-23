#![feature(macro_rules)]
#![feature(phase)]
#[unsafe_destructor]
#[phase(plugin)]

extern crate regex_macros;
extern crate regex;

pub mod bencode;
pub mod torrent;
pub mod tracker;
pub mod tracker_udp;
pub mod upnp;
pub mod util;