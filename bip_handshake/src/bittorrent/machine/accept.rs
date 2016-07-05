use std::any::Any;
use std::net::SocketAddr;

use rotor::{Machine, Response, EventSet, PollOpt, Evented};
use rotor::{Scope, GenericScope, Void};
use rotor_stream::{StreamSocket, Accepted};

use try_accept::TryAccept;

// Copied from https://github.com/tailhook/rotor-stream/blob/master/src/accept.rs and modified.

pub enum Accept<M, A: TryAccept + Sized>
    where A::Output: StreamSocket,
          M: Accepted<Socket = A::Output>
{
    Server(A),
    Connection(M),
}

impl<M, A> Accept<M, A>
    where A: TryAccept<Output = M::Socket> + Evented + Any,
          M: Accepted
{
    pub fn new<S>(sock: A, scope: &mut S) -> Response<Self, Void>
        where S: GenericScope
    {
        match scope.register(&sock, EventSet::readable(), PollOpt::edge()) {
            Ok(()) => {}
            Err(e) => return Response::error(Box::new(e)),
        }

        Response::ok(Accept::Server(sock))
    }
}

impl<M, A> Machine for Accept<M, A>
    where A: TryAccept<Output = M::Socket> + Evented + Any,
          M: Accepted<Seed = SocketAddr>
{
    type Context = M::Context;
    type Seed = (A::Output, SocketAddr);

    fn create((sock, seed): Self::Seed, scope: &mut Scope<Self::Context>) -> Response<Self, Void> {
        M::accepted(sock, seed, scope).wrap(Accept::Connection)
    }

    fn ready(self, events: EventSet, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            Accept::Server(a) => {
                match a.try_accept() {
                    Ok(Some((sock, addr))) => {
                        let seed = (sock, addr);
                        Response::spawn(Accept::Server(a), seed)
                    }
                    Ok(None) => Response::ok(Accept::Server(a)),
                    Err(_) => {
                        // TODO(tailhook) maybe log the error
                        Response::ok(Accept::Server(a))
                    }
                }
            }
            Accept::Connection(m) => {
                m.ready(events, scope)
                 .map(Accept::Connection, |_| unreachable!())
            }
        }
    }

    fn spawned(self, _scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            Accept::Server(a) => {
                match a.try_accept() {
                    Ok(Some((sock, addr))) => {
                        let seed = (sock, addr);
                        Response::spawn(Accept::Server(a), seed)
                    }
                    Ok(None) => Response::ok(Accept::Server(a)),
                    Err(_) => {
                        // TODO(tailhook) maybe log the error
                        Response::ok(Accept::Server(a))
                    }
                }
            }
            Accept::Connection(_) => {
                unreachable!();
            }
        }
    }

    fn timeout(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            Accept::Server(..) => unreachable!(),
            Accept::Connection(m) => m.timeout(scope).map(Accept::Connection, |_| unreachable!()),
        }
    }

    fn wakeup(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            me @ Accept::Server(..) => Response::ok(me),
            Accept::Connection(m) => m.wakeup(scope).map(Accept::Connection, |_| unreachable!()),
        }
    }
}
