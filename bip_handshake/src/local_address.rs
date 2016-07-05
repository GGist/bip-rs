use std::net::SocketAddr;
use std::io;

use rotor::mio::tcp::TcpListener;

/// Trait for discovering the local address that a socket/listener is bound to.
pub trait LocalAddress {
    /// Attempt to get the locally bound address.
    fn local_address(&self) -> io::Result<SocketAddr>;
}

impl LocalAddress for TcpListener {
    fn local_address(&self) -> io::Result<SocketAddr> {
        self.local_addr()
    }
}
