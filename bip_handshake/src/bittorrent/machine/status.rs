use std::sync::mpsc::{self, Sender, Receiver};

use rotor::mio::PollOpt;
use rotor::{Machine, Response, Scope, EventSet, Void};
use rotor_stream::{Accepted, Protocol, Stream, MigrateProtocol};

use bittorrent::handshake::HandshakeSeed;
use bittorrent::seed::{InitiateSeed, CompleteSeed};

// Final composed state machine should look something like:
//
// Accept<Initiate<PeerHandshake<TcpStream, PeerConnection::Context>, PeerConnection<TcpStream>>, TcpListener>
// Accept<Initiate<PeerHandshake<UtpStream, PeerConnection::Context>, PeerConnection<UtpStream>>, UtpListener>

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
    Handshaking(Stream<H>, Receiver<C::Seed>),
    Connected(Stream<C>),
}

impl<H, C> PeerStatus<H, C>
    where H: Protocol<Context = C::Context, Seed = (HandshakeSeed, Sender<C::Seed>), Socket = C::Socket>,
          C: Protocol
{
    fn new(seed: HandshakeSeed, sock: C::Socket, scope: &mut Scope<<Self as Machine>::Context>) -> Response<Self, Void> {
        let (send, recv) = mpsc::channel();

        Stream::new(sock, (seed, send), scope).wrap(|stream| PeerStatus::Handshaking(stream, recv))
    }

    /// Creates a PeerStatus over the Connected protocol with the given arguments.
    pub fn connected(stream: Stream<H>, seed: C::Seed, scope: &mut Scope<<Self as Machine>::Context>) -> Response<Self, Void> {
        stream.migrate(seed, scope).wrap(PeerStatus::Connected)
    }

    /// Creates a PeerStatus over the Handshake protocol and tell the protocol that it is initiating the connection.
    pub fn initiate(seed: InitiateSeed, sock: C::Socket, scope: &mut Scope<<Self as Machine>::Context>) -> Response<Self, Void> {
        PeerStatus::new(HandshakeSeed::Initiate(seed), sock, scope)
    }

    /// Creates a PeerStatus over the Handshake protocol and tell the protocol that is is completing the connection.
    pub fn complete(seed: CompleteSeed, sock: C::Socket, scope: &mut Scope<<Self as Machine>::Context>) -> Response<Self, Void> {
        PeerStatus::new(HandshakeSeed::Complete(seed), sock, scope)
    }
}

/// Aggressively checks to see if a protocol migration from H -> C can occur and, if so, performs the migration.
fn try_protocol_migration<H, C, F>(stream: Stream<H>,
                                   recv: Receiver<C::Seed>,
                                   scope: &mut Scope<H::Context>,
                                   event: F)
                                   -> Response<PeerStatus<H, C>, Void>
    where H: Protocol<Context = C::Context, Seed = (HandshakeSeed, Sender<C::Seed>), Socket = C::Socket>,
          C: Protocol,
          F: FnOnce(Stream<H>, &mut Option<Stream<H>>, &mut Scope<H::Context>) -> Response<(), Void>
{
    let mut opt_stream = None;
    let response = event(stream, &mut opt_stream, scope);

    match (opt_stream, recv.try_recv()) {
        (Some(stream), Ok(seed)) => PeerStatus::connected(stream, seed, scope).map(|c| c, |_| unreachable!()),
        (Some(stream), Err(_)) => Response::ok(PeerStatus::Handshaking(stream, recv)),
        (None, _) => response.map(|_| unreachable!(), |_| unreachable!()),
    }
}

// When a handshaking peer says it is done, that means the handshaking succeeded; we should inject our saved seed to switch our protocol within the
// same state machine. Since we know we are working with Stream connections, we can safely map any Seeds as unreachable since they never originate
// from Streams themselves. If a handshaker returns an error, we let the state machine handle shutting it down as that means something was wrong
// with the handshaking process.
impl<H, C> Machine for PeerStatus<H, C>
    where H: Protocol<Context = C::Context, Seed = (HandshakeSeed, Sender<C::Seed>), Socket = C::Socket>,
          C: Protocol
{
    type Context = H::Context;
    type Seed = Void;

    fn create(_seed: Self::Seed, _scope: &mut Scope<Self::Context>) -> Response<Self, Void> {
        unreachable!()
    }

    fn ready(self, events: EventSet, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, r) => {
                try_protocol_migration(h, r, scope, |stream, opt_stream, scope| {
                    stream.ready(events, scope).map(|s| {
                                                        *opt_stream = Some(s);
                                                    },
                                                    |_| unreachable!())
                })
            }
            PeerStatus::Connected(c) => c.ready(events, scope).map(PeerStatus::Connected, |_| unreachable!()),
        }
    }

    fn spawned(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, r) => {
                try_protocol_migration(h, r, scope, |stream, opt_stream, scope| {
                    stream.spawned(scope).map(|s| {
                                                  *opt_stream = Some(s);
                                              },
                                              |_| unreachable!())
                })
            }
            PeerStatus::Connected(c) => c.spawned(scope).map(PeerStatus::Connected, |_| unreachable!()),
        }
    }

    fn timeout(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, r) => {
                try_protocol_migration(h, r, scope, |stream, opt_stream, scope| {
                    stream.timeout(scope).map(|s| {
                                                  *opt_stream = Some(s);
                                              },
                                              |_| unreachable!())
                })
            }
            PeerStatus::Connected(c) => c.timeout(scope).map(PeerStatus::Connected, |_| unreachable!()),
        }
    }

    fn wakeup(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            PeerStatus::Handshaking(h, r) => {
                try_protocol_migration(h, r, scope, |stream, opt_stream, scope| {
                    stream.wakeup(scope).map(|s| {
                                                 *opt_stream = Some(s);
                                             },
                                             |_| unreachable!())
                })
            }
            PeerStatus::Connected(c) => c.wakeup(scope).map(PeerStatus::Connected, |_| unreachable!()),
        }
    }
}
