use std::cell::RefCell;
use std::rc::Rc;

use rotor::{Machine, Response, Scope, EventSet, Void};
use rotor_stream::{Accepted, Protocol, Stream};

use bittorrent::seed::{InitiateSeed, CompleteSeed};
use try_clone::TryClone;

// Final composed state machine should look something like:
//
// Accept<Initiate<PeerStatus<Stream<PeerHandshake<TcpStream>>, Stream<PeerConnection<TcpStream>>>>, TcpListener>
// Accept<Initiate<PeerStatus<Stream<PeerHandshake<UtpStream>>, Stream<PeerConnection<UtpStream>>>>, UtpListener>

pub enum HandshakeState {
    Initiate(InitiateSeed),
    Complete(CompleteSeed),
}

/// Holds either a handshaking state machine or a connected state machine.
///
/// Connections will start out over the handshake protocol but after that protocol
/// gives a done signal, the connection will migrate over to the connected protocol.
pub enum PeerStatus<H, C>
    where H: Protocol,
          C: Protocol
{
    // Currently rotor will not allow us to pull the C::Socket out from
    // a state machine when it is shutting down, so to maintain the socket
    // when transitioning into C, we need to copy it and store it here.
    Handshaking(Stream<H>, C::Socket, Rc<RefCell<C::Seed>>),
    Connected(Stream<C>),
}

impl<H, C> PeerStatus<H, C>
    where H: Protocol<Context = C::Context, Seed = (HandshakeState, Rc<RefCell<C::Seed>>), Socket = C::Socket>,
          C: Protocol,
          C::Seed: Default,
          C::Socket: TryClone
{
    fn new(state: HandshakeState, sock: C::Socket, scope: &mut Scope<<Self as Machine>::Context>) -> Response<Self, Void> {
        let sock_clone = clone_socket(&sock);
        let rc_seed = Rc::new(RefCell::new(C::Seed::default()));

        Stream::new(sock, (state, rc_seed.clone()), scope).wrap(|stream| PeerStatus::Handshaking(stream, sock_clone, rc_seed))
    }

    /// Creates a PeerStatus over the Connected protocol with the given arguments.
    pub fn connected(sock: C::Socket,
                     rc_seed: Rc<RefCell<C::Seed>>,
                     scope: &mut Scope<<Self as Machine>::Context>)
                     -> Response<Self, Void> {
        let seed = Rc::try_unwrap(rc_seed)
                       .map_err(|_| ())
                       .expect("bip_peer: PeerStatus Failed To Own Rc PeerStatus::Connected Seed")
                       .into_inner();

        Stream::connected(sock, seed, scope).wrap(PeerStatus::Connected)
    }

    /// Creates a PeerStatus over the Handshake protocol and tell the protocol that it is initiating the connection.
    pub fn initiate(seed: InitiateSeed, sock: C::Socket, scope: &mut Scope<<Self as Machine>::Context>) -> Response<Self, Void> {
        PeerStatus::new(HandshakeState::Initiate(seed), sock, scope)
    }

    /// Creates a PeerStatus over the Handshake protocol and tell the protocol that is is completing the connection.
    pub fn complete(seed: CompleteSeed, sock: C::Socket, scope: &mut Scope<<Self as Machine>::Context>) -> Response<Self, Void> {
        PeerStatus::new(HandshakeState::Complete(seed), sock, scope)
    }
}

/// Try to clone the given socket T and panic if an error occurs.
fn clone_socket<T>(sock: &T) -> T
    where T: TryClone
{
    sock.try_clone()
        .expect("bip_peer: PeerStatus Failed To Clone Handshaker Socket")
}

// When a handshaking peer says it is done, that means the handshaking succeeded; we should inject our saved seed to switch our protocol within the
// same state machine. Since we know we are working with Stream connections, we can safely map any Seeds as unreachable since they never originate
// from Streams themselves. If a handshaker returns an error, we let the state machine handle shutting it down as that means something was wrong
// with the handshaking process.
impl<H, C> Machine for PeerStatus<H, C>
    where H: Protocol<Context = C::Context, Seed = (HandshakeState, Rc<RefCell<C::Seed>>), Socket = C::Socket>,
          C: Protocol,
          C::Seed: Default,
          C::Socket: TryClone
{
    type Context = H::Context;
    type Seed = Void;

    fn create(_seed: Self::Seed, _scope: &mut Scope<Self::Context>) -> Response<Self, Void> {
        unreachable!()
    }

    fn ready(self, events: EventSet, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, s, r) => {
                let response = h.ready(events, scope);

                if is_done(&response) {
                    PeerStatus::connected(s, r, scope).map(|c| c, |_| unreachable!())
                } else {
                    response.map(|h| PeerStatus::Handshaking(h, s, r), |_| unreachable!())
                }
            }
            PeerStatus::Connected(c) => c.ready(events, scope).map(PeerStatus::Connected, |_| unreachable!()),
        }
    }

    fn spawned(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, s, r) => {
                let response = h.spawned(scope);

                if is_done(&response) {
                    PeerStatus::connected(s, r, scope).map(|c| c, |_| unreachable!())
                } else {
                    response.map(|h| PeerStatus::Handshaking(h, s, r), |_| unreachable!())
                }
            }
            PeerStatus::Connected(c) => c.spawned(scope).map(PeerStatus::Connected, |_| unreachable!()),
        }
    }

    fn timeout(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, s, r) => {
                let response = h.timeout(scope);

                if is_done(&response) {
                    PeerStatus::connected(s, r, scope).map(|c| c, |_| unreachable!())
                } else {
                    response.map(|h| PeerStatus::Handshaking(h, s, r), |_| unreachable!())
                }
            }
            PeerStatus::Connected(c) => c.timeout(scope).map(PeerStatus::Connected, |_| unreachable!()),
        }
    }

    fn wakeup(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, s, r) => {
                let response = h.wakeup(scope);

                if is_done(&response) {
                    PeerStatus::connected(s, r, scope).map(|c| c, |_| unreachable!())
                } else {
                    response.map(|h| PeerStatus::Handshaking(h, s, r), |_| unreachable!())
                }
            }
            PeerStatus::Connected(c) => c.wakeup(scope).map(PeerStatus::Connected, |_| unreachable!()),
        }
    }
}

/// Return true if the given response is determined to be in a Done state.
fn is_done<M, N>(response: &Response<M, N>) -> bool {
    response.is_stopped() && response.cause().is_none()
}
