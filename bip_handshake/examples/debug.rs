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

struct HelloWorldContext {
    send: Sender<String>
}

struct HelloWorldProtocol;

impl PeerProtocol for HelloWorldProtocol {
    type Context = HelloWorldContext;
    type Protocol = Self;
    type Listener = TcpListener;
    type Socket = TcpStream;
}

impl Protocol for HelloWorldProtocol {
    type Context = BTContext<HelloWorldContext>;
    type Socket = TcpStream;
    type Seed = BTSeed;

    fn create(seed: Self::Seed, sock: &mut Self::Socket, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        println!("ASD");
        scope.notifier().wakeup().unwrap();

        Intent::of(HelloWorldProtocol).sleep()
    }

    fn bytes_read(self, transport: &mut Transport<Self::Socket>, end: usize, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        let message = String::from_utf8(transport.input()[..].to_vec()).unwrap();
        scope.send.send(message).unwrap();

        Intent::done()
    }

    fn bytes_flushed(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        Intent::of(HelloWorldProtocol).expect_delimiter(b"\n", 128)
    }

    fn timeout(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        unimplemented!()
    }

    fn exception(self, _transport: &mut Transport<Self::Socket>, reason: Exception, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        unimplemented!()
    }

    fn fatal(self, reason: Exception, scope: &mut Scope<Self::Context>) -> Option<Box<Error>> {
        unimplemented!()
    }

    fn wakeup(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        transport.output().write_all(&b"Hello World!\n"[..]).unwrap();

        Intent::of(HelloWorldProtocol).expect_flush()
    }
}

fn main() {
    let (m_send, m_recv): (Sender<()>, Receiver<()>) = mpsc::channel();

    let (send_one, recv_one) = mpsc::channel();
    let context_one = HelloWorldContext{ send: send_one };
    let mut handshaker_one = BTHandshaker::new::<HelloWorldProtocol>(m_send.clone(), "127.0.0.1:0".parse().unwrap(), [0u8; 20].into(), context_one).unwrap();

    let (send_two, recv_two) = mpsc::channel();
    let context_two = HelloWorldContext{ send: send_two };
    let mut handshaker_two = BTHandshaker::new::<HelloWorldProtocol>(m_send, "127.0.0.1:0".parse().unwrap(), [1u8; 20].into(), context_two).unwrap();

    handshaker_one.register([0u8; 20].into());
    handshaker_two.register([0u8; 20].into());

    handshaker_one.connect(Some([1u8; 20].into()), [0u8; 20].into(), SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), handshaker_two.port())));

    let message_one = recv_one.recv().unwrap();
    let message_two = recv_two.recv().unwrap();

    assert_eq!(message_one, "Hello World!\n");
    assert_eq!(message_two, "Hello World!\n");
}