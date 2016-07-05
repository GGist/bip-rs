use std::io;
use std::net::SocketAddr;

use rotor::mio::tcp::{TcpListener, TcpStream};

/// Trait for non blocking accepts on socket listeners.
pub trait TryAccept {
    type Output;

    /// Attempt to accept a socket connection.
    fn try_accept(&self) -> io::Result<Option<(Self::Output, SocketAddr)>>;
}

impl TryAccept for TcpListener {
    type Output = TcpStream;

    fn try_accept(&self) -> io::Result<Option<(Self::Output, SocketAddr)>> {
        self.accept()
    }
}
