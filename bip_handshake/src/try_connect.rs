use std::io;
use std::net::SocketAddr;

use rotor::mio::tcp::TcpStream;

/// Trait for non blocking connects on sockets.
pub trait TryConnect: Sized {
    /// Attempt to connect to the given address.
    fn try_connect(addr: SocketAddr) -> io::Result<Self>;
}

impl TryConnect for TcpStream {
    fn try_connect(addr: SocketAddr) -> io::Result<Self> {
        TcpStream::connect(&addr)
    }
}
