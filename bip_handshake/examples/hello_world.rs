extern crate bip_handshake;
extern crate rotor;
extern crate rotor_stream;

use std::sync::mpsc::{self, Sender, Receiver};
use std::error::{Error};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::io::Write;

use rotor::{Scope};
use rotor::mio::tcp::{TcpStream, TcpListener};
use rotor_stream::{Protocol, Intent, Transport, Exception};

use bip_handshake::{PeerProtocol, Handshaker, BTSeed, BTContext, BTHandshaker};

/// Context that we can create and pass in to access from within our protocol.
struct HelloWorldContext {
    send: Sender<String>,
    message: Vec<u8>
}

/// Type that we will implement a PeerProtocol on.
///
/// This would usually contain information related to the state of the protocol
/// such as an enum denoting the state of the connection or other data created
/// during Protocol::create.
struct HelloWorldProtocol;

/// Implement PeerProtocol for our HelloWorldProtocol and provide type information
/// that will allow our handshaker to initialize and create connections over our
/// desired transport (in this case, tcp).
impl PeerProtocol for HelloWorldProtocol {
    type Context = HelloWorldContext;
    type Protocol = Self;
    type Listener = TcpListener;
    type Socket = TcpStream;
}

/// Implement the methods from rotor_stream that will be dispatched when io events occur.
impl Protocol for HelloWorldProtocol {
    type Context = BTContext<HelloWorldContext>;
    type Socket = TcpStream;
    type Seed = BTSeed;

    fn create(_seed: Self::Seed, _sock: &mut Self::Socket, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        // To access the Transport for writing our message, we will go ahead trigger a wakeup
        scope.notifier().wakeup().unwrap();

        // Sleep because we will get the wakeup afterwards
        Intent::of(HelloWorldProtocol).sleep()
    }

    fn bytes_read(self, transport: &mut Transport<Self::Socket>, _end: usize, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        // Place the message in a String and send it to our Receiver channel
        let message = String::from_utf8(transport.input()[..].to_vec()).unwrap();
        scope.send.send(message).unwrap();
        
        // The protocol has completed, initiate teardown
        Intent::done()
    }

    fn bytes_flushed(self, _transport: &mut Transport<Self::Socket>, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        // Expect to read a message delimited with a \n from the remote peer
        Intent::of(HelloWorldProtocol).expect_delimiter(b".", 128)
    }

    fn timeout(self, _transport: &mut Transport<Self::Socket>, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        unimplemented!()
    }

    fn exception(self, _transport: &mut Transport<Self::Socket>, _reason: Exception, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        unimplemented!()
    }

    fn fatal(self, _reason: Exception, _scope: &mut Scope<Self::Context>) -> Option<Box<Error>> {
        unimplemented!()
    }

    fn wakeup(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        // Write out to the peer this message
        transport.output().write_all(&scope.message[..]).unwrap();

        // Wait until our message is written out
        Intent::of(HelloWorldProtocol).expect_flush()
    }
}

fn main() {
    // Setup channels for metadata (there is no metadata in this example, so these are unused internally)
    let (m_send, _m_recv): (Sender<()>, Receiver<()>) = mpsc::channel();

    // Create the first handshaker and place a sender in the peer protocol context
    let (send_one, recv_one) = mpsc::channel();
    let context_one = HelloWorldContext{ send: send_one, message: b"Hello From Handshaker One.".to_vec() };
    let mut handshaker_one = BTHandshaker::new::<HelloWorldProtocol>(m_send.clone(), "127.0.0.1:0".parse().unwrap(), [0u8; 20].into(), context_one).unwrap();

    // Create the second handshaker and place a sender in the peer protocol context
    let (send_two, recv_two) = mpsc::channel();
    let context_two = HelloWorldContext{ send: send_two, message: b"Hello From Handshaker Two.".to_vec() };
    let handshaker_two = BTHandshaker::new::<HelloWorldProtocol>(m_send, "127.0.0.1:0".parse().unwrap(), [1u8; 20].into(), context_two).unwrap();

    // Make sure both handshakers are looking for this InfoHash
    handshaker_one.register([0u8; 20].into());
    handshaker_two.register([0u8; 20].into());

    // Tell handshaker one to initiate a handshake with the given address for the given InfoHash and expect the given PeerId (or close the connection)
    handshaker_one.connect(Some([1u8; 20].into()), [0u8; 20].into(), SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), handshaker_two.port())));

    // Receive the result from both of our handshakers
    let message_one = recv_one.recv().unwrap();
    let message_two = recv_two.recv().unwrap();

    // Assert that each handshaker received the correct message
    println!("Handshaker One Received The Message: {}", message_one);
    println!("Handshaker Two Received The Message: {}", message_two);
}