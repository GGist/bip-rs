extern crate bip_util;
extern crate futures;
#[macro_use]
extern crate nom;
extern crate tokio_core;

mod bittorrent;
mod local_addr;
mod remote_addr;
mod transport;

pub type Protocol = String;

pub use bip_util::bt::{PeerId, InfoHash};