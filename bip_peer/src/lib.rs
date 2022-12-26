#[macro_use]
extern crate bip_bencode;
extern crate bip_handshake;
extern crate bip_util;
extern crate bytes;
extern crate byteorder;
extern crate crossbeam;
#[macro_use]
extern crate error_chain;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_timer;
#[macro_use]
extern crate nom;

#[macro_use]
mod macros;

mod codec;
mod manager;
mod message;
mod protocol;

pub use crate::codec::PeerProtocolCodec;
pub use crate::protocol::{PeerProtocol, NestedPeerProtocol};
pub use crate::manager::{ManagedMessage, PeerManager, PeerManagerSink, PeerManagerStream, IPeerManagerMessage, OPeerManagerMessage, MessageId};
pub use crate::manager::builder::PeerManagerBuilder;
pub use crate::manager::peer_info::PeerInfo;

/// Serializable and deserializable protocol messages.
pub mod messages {
    /// Builder types for protocol messages.
    pub mod builders {
        pub use crate::message::{ExtendedMessageBuilder};
    }

    pub use crate::message::{BitFieldIter, BitFieldMessage, CancelMessage, ExtendedMessage, HaveMessage, PieceMessage, PortMessage,
        RequestMessage, UtMetadataRequestMessage, UtMetadataDataMessage, UtMetadataRejectMessage, BitsExtensionMessage, ExtendedType,
        NullProtocolMessage, PeerExtensionProtocolMessage, PeerWireProtocolMessage, UtMetadataMessage};
}

/// `PeerManager` error types.
pub mod error {
    pub use crate::manager::error::{PeerManagerError, PeerManagerErrorKind, PeerManagerResultExt, PeerManagerResult};
}

/// Implementations of `PeerProtocol`.
pub mod protocols {
    pub use crate::protocol::unit::UnitProtocol;
    pub use crate::protocol::null::NullProtocol;
    pub use crate::protocol::wire::PeerWireProtocol;
    pub use crate::protocol::extension::PeerExtensionProtocol;
}