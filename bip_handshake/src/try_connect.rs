use std::io;
use std::net::SocketAddr;

use rotor::mio::tcp::TcpStream;

pub trait TryConnect: Sized {
    fn connect(addr: SocketAddr) -> io::Result<Self>;
}

impl TryConnect for TcpStream {
    fn connect(addr: SocketAddr) -> io::Result<Self> {
        TcpStream::connect(&addr)
    }
}
