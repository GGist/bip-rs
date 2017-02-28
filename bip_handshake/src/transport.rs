use std::io;
use std::net::SocketAddr;

use local_addr::LocalAddr;
use remote_addr::RemoteAddr;

use futures::future::Future;
use futures::stream::Stream;
use tokio_core::io::Io;
use tokio_core::net::{TcpStream, TcpStreamNew, Incoming, TcpListener};
use tokio_core::reactor::Handle;

/// Trait for connecting to a client overs some generic transport.
pub trait Transport {
    /// Concrete socket.
    type Socket: Io + LocalAddr + RemoteAddr;

    /// Future `Self::Socket`.
    type FutureSocket: Future<Item=Self::Socket, Error=io::Error>;

    /// Concrete listener.
    type Listener: Stream<Item=(Self::Socket, SocketAddr), Error=io::Error>;

    /// Connect to the given address over this transport, using the supplied `Handle`.
    fn connect(addr: &SocketAddr, handle: &Handle) -> io::Result<Self::FutureSocket>;

    /// Listen to the given address for this transport, using the supplied `Handle`.
    fn listen(addr: &SocketAddr, handle: &Handle) -> io::Result<Self::Listener>;
}

impl Transport for TcpStream {
    type Socket = TcpStream;
    type FutureSocket = TcpStreamNew;
    type Listener = Incoming;

    fn connect(addr: &SocketAddr, handle: &Handle) -> io::Result<TcpStreamNew> {
        Ok(TcpStream::connect(addr, handle))
    }

    fn listen(addr: &SocketAddr, handle: &Handle) -> io::Result<Incoming> {
        TcpListener::bind(addr, handle).and_then(|listener| Ok(listener.incoming()))
    }
}