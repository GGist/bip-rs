use std::any::Any;

use rotor::mio::Evented;
use rotor_stream::{Protocol, StreamSocket};

use bittorrent::handshake::context::BTContext;
use bittorrent::seed::BTSeed;
use local_address::LocalAddress;
use try_accept::TryAccept;
use try_bind::TryBind;
use try_clone::TryClone;
use try_connect::TryConnect;

/// Trait for plugging in custom peer protocols.
pub trait PeerProtocol {
    /// Context that the peer protocol can access.
    type Context;

    /// Protocol that includes all methods supplied by the custom protocol.
    type Protocol: Protocol<Context = BTContext<Self::Context>, Seed = BTSeed, Socket = Self::Socket> + Send;

    /// Listener that can yield sockets expected by the given protocol.
    type Listener: LocalAddress + TryBind + TryAccept<Output = Self::Socket> + Evented + Any + Send;

    /// Socket that is stream oriented and can be cloned.
    type Socket: TryConnect + TryClone + StreamSocket + Send;
}
