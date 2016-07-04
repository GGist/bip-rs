use std::sync::mpsc::Receiver;
use std::cell::RefCell;
use std::rc::Rc;
use std::net::SocketAddr;

use rotor::{Void, Scope, Response, Machine, EventSet};
use rotor::mio::tcp::TcpStream;
use rotor_stream::{Protocol, Accepted};

use bittorrent::machine::status::{PeerStatus, HandshakeState};
use bittorrent::seed::{CompleteSeed, InitiateSeed};
use try_clone::TryClone;
use try_connect::TryConnect;

pub enum Initiate<H, C>
    where H: Protocol,
          C: Protocol
{
    Peer(PeerStatus<H, C>),
    Recv(Receiver<InitiateSeed>),
}

impl<H, C> Initiate<H, C>
    where H: Protocol,
          C: Protocol,
          C::Socket: TryConnect
{
    /// Try to receive an initiation seed from the given receiver.
    ///
    /// If a seed is received, a connection will be attempted and
    /// if successful, a new Peer state machine will be spawned.
    fn try_receive(recv: Receiver<InitiateSeed>) -> Response<Self, (C::Socket, InitiateSeed)> {
        let opt_seed = recv.try_recv().ok().and_then(|init| C::Socket::connect(init.addr()).ok().map(|stream| (stream, init)));

        let self_recv = Initiate::Recv(recv);
        if let Some(seed) = opt_seed {
            Response::spawn(self_recv, seed)
        } else {
            Response::ok(self_recv)
        }
    }
}

impl<H, C> Accepted for Initiate<H, C>
    where H: Protocol<Context = C::Context, Seed = (HandshakeState, Rc<RefCell<C::Seed>>), Socket = C::Socket>,
          C: Protocol,
          C::Seed: Default,
          C::Socket: TryClone + TryConnect
{
    type Seed = SocketAddr;
    type Socket = C::Socket;

    fn accepted(sock: Self::Socket, seed: SocketAddr, scope: &mut Scope<Self::Context>) -> Response<Self, Void> {
        PeerStatus::complete(CompleteSeed::new(seed), sock, scope).wrap(Initiate::Peer)
    }
}

impl<H, C> Machine for Initiate<H, C>
    where H: Protocol<Context = C::Context, Seed = (HandshakeState, Rc<RefCell<C::Seed>>), Socket = C::Socket>,
          C: Protocol,
          C::Seed: Default,
          C::Socket: TryClone + TryConnect
{
    type Context = H::Context;
    type Seed = (C::Socket, InitiateSeed);

    fn create((sock, seed): Self::Seed, scope: &mut Scope<Self::Context>) -> Response<Self, Void> {
        PeerStatus::initiate(seed, sock, scope).wrap(Initiate::Peer)
    }

    fn ready(self, events: EventSet, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            Initiate::Peer(p) => p.ready(events, scope).map(Initiate::Peer, |_| unreachable!()),
            Initiate::Recv(_) => unreachable!(),
        }
    }

    fn spawned(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            Initiate::Peer(_) => unreachable!(),
            Initiate::Recv(r) => Initiate::try_receive(r),
        }
    }

    fn timeout(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            Initiate::Peer(p) => p.timeout(scope).map(Initiate::Peer, |_| unreachable!()),
            Initiate::Recv(_) => unreachable!(),
        }
    }

    fn wakeup(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            Initiate::Peer(p) => p.wakeup(scope).map(Initiate::Peer, |_| unreachable!()),
            Initiate::Recv(r) => Initiate::try_receive(r),
        }
    }
}
