use std::cell::RefCell;
use std::rc::Rc;

use rotor::{Machine, Response, Scope, EventSet, Void};
use rotor_stream::{Accepted, Protocol, Stream};

/// Composes two peer state machines including one for handshaking and one for connections.
///
/// Intended to be composed with an `Accept` state machine to accept the peer connections.
enum PeerStatus<H, C> where H: Protocol, C: Protocol {
    Handshaking(Stream<H>, Rc<RefCell<(C::Socket, C::Seed)>>),
    Connected(Stream<C>)
}

// Accept<PeerStatus<Stream<PeerHandshake<TcpStream>>, Stream<PeerConnection<TcpStream>>>, TcpListener>
// Accept<PeerStatus<Stream<PeerHandshake<UtpStream>>, Stream<PeerConnection<UtpStream>>>, UtpListener>

impl<H, C> Accepted for PeerStatus<H, C> where H: Protocol<Context=C::Context, Seed=Rc<RefCell<(C::Socket, C::Seed)>>, Socket=C::Socket>, C: Protocol, H::Seed: Clone {
    type Seed = Rc<RefCell<(C::Socket, C::Seed)>>;
    type Socket = C::Socket;

    fn accepted(sock: C::Socket, rc_seed: Rc<RefCell<(C::Socket, C::Seed)>>, scope: &mut Scope<Self::Context>) -> Response<Self, Void> {
        Stream::new(sock, rc_seed.clone(), scope).wrap(|stream| PeerStatus::Handshaking(stream, rc_seed))
    }
}

// When a handshaking peer says it is done, that means the handshaking succeeded; we should inject our saved seed to switch our protocol within the
// same state machine. Since we know we are working with Stream connections, we can safely map any Seeds as unreachable since they never originate
// from Streams themselves. If a handshaker returns an error, we let the state machine handle shutting it down as that means something was wrong
// with the handshaking process.
impl<H, C> Machine for PeerStatus<H, C> where H: Protocol<Context=C::Context, Seed=Rc<RefCell<(C::Socket, C::Seed)>>, Socket=C::Socket>, C: Protocol {
    type Context = H::Context;
    type Seed = H::Seed;

    fn create(rc_seed: Self::Seed, scope: &mut Scope<Self::Context>) -> Response<Self, Void> {
        let (sock, seed) = Rc::try_unwrap(rc_seed)
            .map_err(|_| ())
            .expect("bip_peer: Failed To Take Ownership Of Rc PeerStatus::Connected Seed")
            .into_inner();

        Stream::connected(sock, seed, scope).map(PeerStatus::Connected, |_| unreachable!())
    }

    fn ready(self, events: EventSet, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, s) => {
                let response = h.ready(events, scope);

                if is_done(&response) {
                    PeerStatus::create(s, scope).map(|c| c, |_| unreachable!())
                } else {
                    response.map(|h| PeerStatus::Handshaking(h, s), |_| unreachable!())
                }
            },
            PeerStatus::Connected(c) => c.ready(events, scope).map(PeerStatus::Connected, |_| unreachable!())
        }
    }

    // Implementation detail, but any H that wishes to transfer into a C should most likely shut
    // itself down when it hits it's own spawned event as we will not be able to do that for them.
    fn spawned(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, s) => {
                let response = h.spawned(scope);

                if is_done(&response) {
                    PeerStatus::create(s, scope).map(|c| c, |_| unreachable!())
                } else {
                    response.map(|h| PeerStatus::Handshaking(h, s), |_| unreachable!())
                }
            },
            PeerStatus::Connected(c) => c.spawned(scope).map(PeerStatus::Connected, |_| unreachable!())
        }
    }

    fn timeout(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, s) => {
                let response = h.timeout(scope);

                if is_done(&response) {
                    PeerStatus::create(s, scope).map(|c| c, |_| unreachable!())
                } else {
                    response.map(|h| PeerStatus::Handshaking(h, s), |_| unreachable!())
                }
            },
            PeerStatus::Connected(c) => c.timeout(scope).map(PeerStatus::Connected, |_| unreachable!())
        }
    }

    fn wakeup(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, s) => {
                let response = h.wakeup(scope);

                if is_done(&response) {
                    PeerStatus::create(s, scope).map(|c| c, |_| unreachable!())
                } else {
                    response.map(|h| PeerStatus::Handshaking(h, s), |_| unreachable!())
                }
            },
            PeerStatus::Connected(c) => c.wakeup(scope).map(PeerStatus::Connected, |_| unreachable!())
        }
    }
}

fn is_done<M, N>(response: &Response<M, N>) -> bool {
    response.is_stopped() && response.cause().is_none()
}