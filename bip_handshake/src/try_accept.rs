use std::io;
use std::net::SocketAddr;

use rotor::mio::tcp::{TcpListener, TcpStream};

pub trait TryAccept {
    type Output;

    fn accept(&self) -> io::Result<Option<(Self::Output, SocketAddr)>>;
}

impl TryAccept for TcpListener {
    type Output = TcpStream;

    fn accept(&self) -> io::Result<Option<(Self::Output, SocketAddr)>> {
        self.accept()
    }
}
