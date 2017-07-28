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

pub use message::complete::CompleteMessage;
pub use message::initiate::InitiateMessage;
pub use message::protocol::Protocol;
pub use message::extensions::{Extensions, Extension};

pub use handshake::config::HandshakerConfig;
pub use handshake::handshaker::{HandshakerBuilder, Handshaker, HandshakerStream, HandshakerSink};

pub use filter::{FilterDecision, HandshakeFilter, HandshakeFilters};

pub use discovery::DiscoveryInfo;
pub use local_addr::LocalAddr;
pub use transport::Transport;

/// Built in objects implementing `Transport`.
pub mod transports {
    pub use transport::{TcpTransport, TcpListenerStream};
}

pub use bip_util::bt::{PeerId, InfoHash};