extern crate bip_util;
extern crate error_chain;
extern crate futures;
#[macro_use]
extern crate nom;
extern crate rand;
extern crate tokio_core;
extern crate tokio_timer;

mod bittorrent;
mod handshake;
mod message;
mod filter;
mod local_addr;
mod remote_addr;
mod transport;

pub use message::complete::CompleteMessage;
pub use message::initiate::InitiateMessage;
pub use message::protocol::Protocol;
pub use message::extensions::Extensions;

pub use handshake::handshaker::{HandshakerBuilder, Handshaker, HandshakerStream, HandshakerSink};

pub use filter::{FilterDecision, HandshakeFilter, HandshakeFilters};

pub use local_addr::LocalAddr;
pub use remote_addr::RemoteAddr;
pub use transport::Transport;

pub use bip_util::bt::{PeerId, InfoHash};
/*
pub use bittorrent::protocol::extensions::Extensions;
pub use bittorrent::handshake::handshaker::{HandshakerBuilder, Handshaker, HandshakerStream, HandshakerSink};
pub use bittorrent::handshake::initiate::InitiateHandshake;
pub use bittorrent::handshake::complete::CompleteHandshake;

pub mod filters {
    pub use filter::{BittorrentFilter, SocketBlockFilter, InfoHashAllowFilter};
}

pub mod extension {
    
}

pub use filter::{HandshakeFilters, HandshakeFilter, FilterDecision};
pub use local_addr::LocalAddr;
pub use remote_addr::RemoteAddr;
pub use transport::Transport;

pub use bip_util::bt::{PeerId, InfoHash};*/