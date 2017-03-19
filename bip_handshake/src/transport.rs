use std::io;
use std::net::SocketAddr;

use local_addr::LocalAddr;

//use futures::Poll;
use futures::future::Future;
use futures::stream::Stream;
//use tokio_core::net::{TcpStream, TcpStreamNew, Incoming, TcpListener};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};

/// Trait for initializing connections over an abstract `Transport`.
pub trait Transport {
    /// Concrete socket.
    type Socket: AsyncRead + AsyncWrite + 'static;

    /// Future `Self::Socket`.
    type FutureSocket: Future<Item=Self::Socket, Error=io::Error> + 'static;

    /// Concrete listener.
    type Listener: Stream<Item=(Self::Socket, SocketAddr), Error=io::Error> + LocalAddr + 'static;

    /// Connect to the given address over this transport, using the supplied `Handle`.
    fn connect(addr: &SocketAddr, handle: &Handle) -> io::Result<Self::FutureSocket>;

    /// Listen to the given address for this transport, using the supplied `Handle`.
    fn listen(addr: &SocketAddr, handle: &Handle) -> io::Result<Self::Listener>;
}

//--------------------------------------------------------------------------//
/*
/// `Incoming` stream that allows retrieval of the `LocalAddr`.
pub struct IncomingWithLocalAddr {
    local_addr: SocketAddr,
    incoming:   Incoming
}

impl Stream for IncomingWithLocalAddr {
    type Item = (TcpStream, SocketAddr);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<(TcpStream, SocketAddr)>, io::Error> {
        self.incoming.poll()
    }
}

impl LocalAddr for IncomingWithLocalAddr {
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local_addr)
    }
}

impl Transport for TcpStream {
    type Socket = TcpStream;
    type FutureSocket = TcpStreamNew;
    type Listener = IncomingWithLocalAddr;

    fn connect(addr: &SocketAddr, handle: &Handle) -> io::Result<TcpStreamNew> {
        Ok(TcpStream::connect(addr, handle))
    }

    fn listen(addr: &SocketAddr, handle: &Handle) -> io::Result<IncomingWithLocalAddr> {
        TcpListener::bind(addr, handle).and_then(|listener| {
            let local_addr = try!(listener.local_addr());
            let incoming = listener.incoming();

            Ok(IncomingWithLocalAddr{ local_addr: local_addr, incoming: incoming })
        })
    }
}*/