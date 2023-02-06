use std::io;
use std::net::SocketAddr;

use crate::local_addr::LocalAddr;

use futures::future::Future;
use futures::stream::Stream;
use futures::Poll;
use tokio_core::net::{Incoming, TcpListener, TcpStream, TcpStreamNew};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};

/// Trait for initializing connections over an abstract `Transport`.
pub trait Transport {
    /// Concrete socket.
    type Socket: AsyncRead + AsyncWrite + 'static;

    /// Future `Self::Socket`.
    type FutureSocket: Future<Item = Self::Socket, Error = io::Error> + 'static;

    /// Concrete listener.
    type Listener: Stream<Item = (Self::Socket, SocketAddr), Error = io::Error>
        + LocalAddr
        + 'static;

    /// Connect to the given address over this transport, using the supplied
    /// `Handle`.
    fn connect(&self, addr: &SocketAddr, handle: &Handle) -> io::Result<Self::FutureSocket>;

    /// Listen to the given address for this transport, using the supplied
    /// `Handle`.
    fn listen(&self, addr: &SocketAddr, handle: &Handle) -> io::Result<Self::Listener>;
}

//----------------------------------------------------------------------------------//

/// Defines a `Transport` operating over TCP.
pub struct TcpTransport;

impl Transport for TcpTransport {
    type Socket = TcpStream;
    type FutureSocket = TcpStreamNew;
    type Listener = TcpListenerStream<Incoming>;

    fn connect(&self, addr: &SocketAddr, handle: &Handle) -> io::Result<Self::FutureSocket> {
        Ok(TcpStream::connect(addr, handle))
    }

    fn listen(&self, addr: &SocketAddr, handle: &Handle) -> io::Result<Self::Listener> {
        let listener = TcpListener::bind(addr, handle)?;
        let listen_addr = listener.local_addr()?;

        Ok(TcpListenerStream::new(listen_addr, listener.incoming()))
    }
}

/// Convenient object that wraps a listener stream `L`, and also implements
/// `LocalAddr`.
pub struct TcpListenerStream<L> {
    listen_addr: SocketAddr,
    listener: L,
}

impl<L> TcpListenerStream<L> {
    fn new(listen_addr: SocketAddr, listener: L) -> TcpListenerStream<L> {
        TcpListenerStream {
            listen_addr,
            listener,
        }
    }
}

impl<L> Stream for TcpListenerStream<L>
where
    L: Stream,
{
    type Item = L::Item;
    type Error = L::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.listener.poll()
    }
}

impl<L> LocalAddr for TcpListenerStream<L> {
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.listen_addr)
    }
}

//----------------------------------------------------------------------------------//

#[cfg(test)]
pub mod test_transports {
    use std::io::{self, Cursor};
    use std::net::SocketAddr;

    use super::Transport;
    use crate::local_addr::LocalAddr;

    use futures::future::{self, FutureResult};
    use futures::stream::{self, Empty, Stream};
    use futures::Poll;
    use tokio_core::reactor::Handle;

    pub struct MockTransport;

    impl Transport for MockTransport {
        type Socket = Cursor<Vec<u8>>;
        type FutureSocket = FutureResult<Self::Socket, io::Error>;
        type Listener = MockListener;

        fn connect(&self, _addr: &SocketAddr, _handle: &Handle) -> io::Result<Self::FutureSocket> {
            Ok(future::ok(Cursor::new(Vec::new())))
        }

        fn listen(&self, addr: &SocketAddr, _handle: &Handle) -> io::Result<Self::Listener> {
            Ok(MockListener::new(*addr))
        }
    }

    //----------------------------------------------------------------------------------//

    pub struct MockListener {
        addr: SocketAddr,
        empty: Empty<(Cursor<Vec<u8>>, SocketAddr), io::Error>,
    }

    impl MockListener {
        fn new(addr: SocketAddr) -> MockListener {
            MockListener {
                addr,
                empty: stream::empty(),
            }
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
