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

pub use codec::PeerProtocolCodec;
pub use protocol::{PeerProtocol, NestedPeerProtocol};
pub use manager::{ManagedMessage, PeerManager, PeerManagerSink, PeerManagerStream, IPeerManagerMessage, OPeerManagerMessage, MessageId};
pub use manager::builder::PeerManagerBuilder;
pub use manager::peer_info::PeerInfo;

/// Serializable and deserializable protocol messages.
pub mod messages {
    /// Builder types for protocol messages.
    pub mod builders {
        pub use message::{ExtendedMessageBuilder};
    }

    pub use message::{BitFieldIter, BitFieldMessage, CancelMessage, ExtendedMessage, HaveMessage, PieceMessage, PortMessage,
        RequestMessage, UtMetadataRequestMessage, UtMetadataDataMessage, UtMetadataRejectMessage, BitsExtensionMessage, ExtendedType,
        NullProtocolMessage, PeerExtensionProtocolMessage, PeerWireProtocolMessage, UtMetadataMessage};
}

/// `PeerManager` error types.
pub mod error {
    pub use manager::error::{PeerManagerError, PeerManagerErrorKind, PeerManagerResultExt, PeerManagerResult};
}

/// Implementations of `PeerProtocol`.
pub mod protocols {
    pub use protocol::unit::UnitProtocol;
    pub use protocol::null::NullProtocol;
    pub use protocol::wire::PeerWireProtocol;
    pub use protocol::extension::PeerExtensionProtocol;
}