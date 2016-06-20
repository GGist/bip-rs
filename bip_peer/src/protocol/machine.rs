use std::sync::mpsc::{Receiver, TryRecvError};

use bip_util::bt::{PeerId, InfoHash};
use bip_util::send::TrySender;
use rotor::{Machine, Void, Scope, Response, EventSet};
use rotor_stream::{Accepted, StreamSocket};

use disk::{InactiveDiskManager, ODiskMessage, ActiveDiskManager, IDiskMessage};
use protocol::OProtocolMessage;
use piece::OSelectorMessage;
use registration::LayerRegistration;

pub struct ProtocolContext {
    disk: Box<LayerRegistration<ODiskMessage, IDiskMessage, SS2 = ActiveDiskManager> + Send>,
    sele: Box<TrySender<OProtocolMessage> + Send>,
}

impl ProtocolContext {
    pub fn new<D, S>(disk: D, selector: S) -> ProtocolContext
        where D: LayerRegistration<ODiskMessage, IDiskMessage, SS2 = ActiveDiskManager> + 'static + Send,
              S: LayerRegistration<OSelectorMessage, OProtocolMessage> + 'static + Send
    {
        // Selector will not send anything through this channel, instead, it will wait to
        // receive a PeerConnect message with a sender for that peer. Peers will send back
        // to the selector through this selector channel (to reduce the number of channels
        // created) and will be dis ambiguated with the PeerIdentifier (corresponds to a unique peer).
        let sel_send = Box::new(selector.register(Box::new(UnusedSender)));

        ProtocolContext {
            disk: Box::new(disk),
            sele: sel_send,
        }
    }

    pub fn register_disk(&self, send: Box<TrySender<ODiskMessage>>) -> ActiveDiskManager {
        self.disk.register(send)
    }

    pub fn send_selector(&self, msg: OProtocolMessage) {
        assert!(self.sele.try_send(msg).is_none());
    }
}

// ----------------------------------------------------------------------------//

struct UnusedSender;

impl TrySender<OSelectorMessage> for UnusedSender {
    fn try_send(&self, msg: OSelectorMessage) -> Option<OSelectorMessage> {
        panic!("bip_peer: Selector Tried To Send Message Through UnusedSender")
    }
}

// ----------------------------------------------------------------------------//

pub enum AcceptPeer<P, C> {
    Shutdown,
    Incoming(Receiver<(P, PeerId, InfoHash)>),
    Connection(C),
}

impl<P, C> Machine for AcceptPeer<P, C>
    where C: Accepted<Socket = P, Seed = (PeerId, InfoHash)>,
          P: StreamSocket
{
    type Context = C::Context;
    type Seed = (P, PeerId, InfoHash);

    fn create((peer, pid, hash): Self::Seed, scope: &mut Scope<Self::Context>) -> Response<Self, Void> {
        C::accepted(peer, (pid, hash), scope).wrap(AcceptPeer::Connection)
    }

    fn ready(self, events: EventSet, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            AcceptPeer::Shutdown => unreachable!(),
            AcceptPeer::Incoming(_) => unreachable!(),
            AcceptPeer::Connection(c) => c.ready(events, scope).map(AcceptPeer::Connection, |_| unreachable!()),
        }
    }

    fn spawned(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            AcceptPeer::Shutdown => unreachable!(),
            AcceptPeer::Incoming(i) => accept_peer(i),
            AcceptPeer::Connection(_) => unreachable!(),
        }
    }

    fn timeout(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            AcceptPeer::Shutdown => unreachable!(),
            AcceptPeer::Incoming(_) => unreachable!(),
            AcceptPeer::Connection(c) => c.timeout(scope).map(AcceptPeer::Connection, |_| unreachable!()),
        }
    }

    fn wakeup(self, scope: &mut Scope<Self::Context>) -> Response<Self, Self::Seed> {
        match self {
            AcceptPeer::Shutdown => {
                scope.shutdown_loop();
                Response::done()
            }
            AcceptPeer::Incoming(i) => accept_peer(i),
            AcceptPeer::Connection(c) => c.wakeup(scope).map(AcceptPeer::Connection, |_| unreachable!()),
        }
    }
}

fn accept_peer<P, C>(recv: Receiver<(P, PeerId, InfoHash)>) -> Response<AcceptPeer<P, C>, (P, PeerId, InfoHash)> {
    match recv.try_recv() {
        Ok((peer, pid, hash)) => Response::spawn(AcceptPeer::Incoming(recv), (peer, pid, hash)),
        Err(TryRecvError::Empty) => Response::ok(AcceptPeer::Incoming(recv)),
        Err(TryRecvError::Disconnected) => panic!("bip_peer: Protocol layer peer receiver disconnect"),
    }
}
