use std::io;
use std::net::SocketAddr;

use local_addr::LocalAddr;
use remote_addr::RemoteAddr;

use futures::future::Future;
use tokio_core::io::Io;
use tokio_core::net::{TcpStream, TcpStreamNew};
use tokio_core::reactor::Handle;

/// Trait for connecting to a client overs some generic transport.
pub trait Transport {
    /// Concrete transport type.
    type Type: Io + LocalAddr + RemoteAddr;
    /// Future yielding `Self::Type` or `io::Error`.
    type FutureType: Future<Item=Self::Type, Error=io::Error>;

    /// Connect to the given address over this transport, using the supplied `Handle`.
    fn connect(addr: &SocketAddr, handle: &Handle) -> Self::FutureType;
}

impl Transport for TcpStream {
    type Type = TcpStream;
    type FutureType = TcpStreamNew;

    fn connect(addr: &SocketAddr, handle: &Handle) -> TcpStreamNew {
        TcpStream::connect(addr, handle)
    }
}