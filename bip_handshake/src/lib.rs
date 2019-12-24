extern crate bip_util;
extern crate bytes;
extern crate futures;
#[macro_use]
extern crate nom;
extern crate rand;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;
extern crate tokio_timer;

mod bittorrent;
mod handshake;
mod message;
mod filter;
mod discovery;
mod local_addr;
mod transport;

pub use crate::message::complete::CompleteMessage;
pub use crate::message::initiate::InitiateMessage;
pub use crate::message::protocol::Protocol;
pub use crate::message::extensions::{Extensions, Extension};

pub use crate::handshake::config::HandshakerConfig;
pub use crate::handshake::handshaker::{HandshakerBuilder, Handshaker, HandshakerStream, HandshakerSink};

pub use crate::filter::{FilterDecision, HandshakeFilter, HandshakeFilters};

pub use crate::discovery::DiscoveryInfo;
pub use crate::local_addr::LocalAddr;
pub use crate::transport::Transport;

/// Built in objects implementing `Transport`.
pub mod transports {
    pub use crate::transport::{TcpTransport, TcpListenerStream};
}

pub use bip_util::bt::{PeerId, InfoHash};