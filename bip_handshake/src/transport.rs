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

#[cfg(test)]
pub mod test_transports {
    use std::io::{self, Cursor};
    use std::net::SocketAddr;

    use super::Transport;
    use local_addr::LocalAddr;

    use futures::{Poll};
    use futures::future::{self, FutureResult};
    use futures::stream::{self, Stream, Empty};
    use tokio_core::reactor::Handle;

    pub struct MockTransport;

    impl Transport for MockTransport {
        type Socket       = Cursor<Vec<u8>>;
        type FutureSocket = FutureResult<Self::Socket, io::Error>;
        type Listener     = MockListener;

        fn connect(_addr: &SocketAddr, _handle: &Handle) -> io::Result<Self::FutureSocket> {
            Ok(future::ok(Cursor::new(Vec::new())))
        }

        fn listen(addr: &SocketAddr, _handle: &Handle) -> io::Result<Self::Listener> {
            Ok(MockListener::new(*addr))
        }
    }

    //----------------------------------------------------------------------------------//

    pub struct MockListener {
        addr: SocketAddr,
        empty: Empty<(Cursor<Vec<u8>>, SocketAddr), io::Error>
    }

    impl MockListener {
        fn new(addr: SocketAddr) -> MockListener {
            MockListener{ addr: addr, empty: stream::empty() }
        }
    }

    impl LocalAddr for MockListener {
        fn local_addr(&self) -> io::Result<SocketAddr> {
            Ok(self.addr)
        }
    }

    impl Stream for MockListener {
        type Item = (Cursor<Vec<u8>>, SocketAddr);
        type Error = io::Error;

        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            self.empty.poll()
        }
    }
}