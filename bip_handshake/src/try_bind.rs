use std::io;
use std::net::SocketAddr;

use rotor::mio::tcp::TcpListener;

/// Trait for non blocking binds on sockets/listeners.
pub trait TryBind: Sized {
    /// Attempt to bind to the given address.
    fn try_bind(addr: SocketAddr) -> io::Result<Self>;
}

impl TryBind for TcpListener {
    fn try_bind(addr: SocketAddr) -> io::Result<Self> {
        TcpListener::bind(&addr)
    }
}
