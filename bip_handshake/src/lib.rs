mod bittorrent;
mod discovery;
mod filter;
mod handshake;
mod local_addr;
mod message;
mod transport;

pub use crate::message::complete::CompleteMessage;
pub use crate::message::extensions::{Extension, Extensions};
pub use crate::message::initiate::InitiateMessage;
pub use crate::message::protocol::Protocol;

pub use crate::handshake::config::HandshakerConfig;
pub use crate::handshake::handshaker::{
    Handshaker, HandshakerBuilder, HandshakerSink, HandshakerStream,
};

pub use crate::filter::{FilterDecision, HandshakeFilter, HandshakeFilters};

pub use crate::discovery::DiscoveryInfo;
pub use crate::local_addr::LocalAddr;
pub use crate::transport::Transport;

/// Built in objects implementing `Transport`.
pub mod transports {
    pub use crate::transport::{TcpListenerStream, TcpTransport};
}

pub use bip_util::bt::{InfoHash, PeerId};
