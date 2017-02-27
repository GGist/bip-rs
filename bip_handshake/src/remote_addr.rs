use std::io;
use std::net::SocketAddr;

use tokio_core::net::TcpStream;

/// Trait for getting the remote address.
pub trait RemoteAddr {
    /// Get the remote address.
    fn remote_addr(&self) -> io::Result<SocketAddr>;
}

impl RemoteAddr for TcpStream {
    fn remote_addr(&self) -> io::Result<SocketAddr> {
        TcpStream::peer_addr(self)
    }
}