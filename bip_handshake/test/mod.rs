extern crate bip_handshake;
extern crate rotor_stream;
extern crate rotor;

use std::sync::mpsc::{self, Sender, Receiver};
use std::error::{Error};

use rotor::{Scope};
use rotor::mio::tcp::{TcpStream, TcpListener};
use rotor_stream::{Protocol, Intent, Transport, Exception};

use bip_handshake::{BTSeed, BTContext};
use bip_handshake::protocol::{PeerProtocol};

mod test_connect;
mod test_connect_any_pid;
mod test_custom_protocol;
mod test_drop;
mod test_drop_clone;
mod test_initiate_wrong_hash;
mod test_receive_wrong_hash;
mod test_wrong_pid;
mod test_wrong_protocol;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum MockEvent {
    Connect,
    Disconnect
}

#[derive(Clone)]
struct MockContext {
    send: Sender<MockEvent>
}

impl MockContext {
    pub fn new() -> (MockContext, Receiver<MockEvent>) {
        let (send, recv) = mpsc::channel();

        (MockContext{ send: send }, recv)
    }
}

struct MockProtocol;

impl PeerProtocol for MockProtocol {
    type Context = MockContext;
    type Protocol = Self;
    type Listener = TcpListener;
    type Socket = TcpStream;
}

impl Protocol for MockProtocol {
    type Context = BTContext<MockContext>;
    type Socket = TcpStream;
    type Seed = BTSeed;

    fn create(_seed: Self::Seed, _sock: &mut Self::Socket, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        // Mock protocol will signal the Sender when the handshake succeeds
        scope.send.send(MockEvent::Connect).unwrap();

        // Sleep so we can allow tests to see if a disconnect occurs (useful if one end of the handshake severes if it reads last)
        Intent::of(MockProtocol).sleep()
    }

    fn bytes_read(self, _transport: &mut Transport<Self::Socket>, _end: usize, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        unimplemented!()
    }

    fn bytes_flushed(self, _transport: &mut Transport<Self::Socket>, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        unimplemented!()
    }

    fn timeout(self, _transport: &mut Transport<Self::Socket>, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        unimplemented!()
    }

    fn exception(self, _transport: &mut Transport<Self::Socket>, _reason: Exception, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        unimplemented!()
    }

    fn fatal(self, reason: Exception, scope: &mut Scope<Self::Context>) -> Option<Box<Error>> {
        match reason {
            Exception::EndOfStream => scope.send.send(MockEvent::Disconnect).unwrap(),
            _ => unimplemented!()
        };

        None
    }

    fn wakeup(self, _transport: &mut Transport<Self::Socket>, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        unimplemented!()
    }
}