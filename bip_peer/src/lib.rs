extern crate bip_util;
extern crate bytes;
extern crate byteorder;
#[macro_use]
extern crate error_chain;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_timer;
#[macro_use]
extern crate nom;

pub mod message;

mod codec;
mod manager;
mod protocol;

pub use codec::PeerProtocolCodec;
pub use protocol::PeerProtocol;
pub use manager::{ManagedMessage, PeerManager, IPeerManagerMessage, OPeerManagerMessage, MessageId};
pub use manager::builder::PeerManagerBuilder;
pub use manager::peer_info::PeerInfo;

/// `PeerManager` error types.
pub mod error {
    pub use manager::error::{PeerManagerError, PeerManagerErrorKind, PeerManagerResultExt, PeerManagerResult};
}

/// Implementations of `PeerProtocol`.
pub mod protocols {
    pub use protocol::null::NullProtocol;
    pub use protocol::wire::PeerWireProtocol;
}