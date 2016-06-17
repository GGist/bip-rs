use std::io;
use std::sync::mpsc::{self, SyncSender};
use std::thread;

use bip_handshake::BTPeer;
use bip_util::bt::{PeerId, InfoHash};
use bip_util::sender::{Sender, PrioritySender};
use rotor::{Notifier, Loop, Config, Response};
use rotor::mio::tcp::TcpStream;
use rotor_stream::Stream;

use disk::{InactiveDiskManager, ODiskMessage, IDiskMessage, ActiveDiskManager};
use piece::{PieceSelector, OSelectorMessage};
use protocol::{IProtocolMessage, ProtocolSender, OProtocolMessage};
use protocol::machine::{ProtocolContext, AcceptPeer};
use protocol::tcp::peer::PeerConnection;
use registration::LayerRegistration;

mod peer;

// TODO: Drop peers that are trying to connect within the AcceptPeer machine.
// This would require holding and updating an AtomicUsize in the machine context.
// Two slots are used by both the shutdown and peer receives state machines.
const MAX_CONNECTED_PEERS: usize = 8192;

const MAX_PENDING_NEW_PEERS: usize = 64;

struct TCPProtocol {
    shutdown: Notifier,
    peer_send: PeerSender,
}

impl TCPProtocol {
    pub fn new<D, S>(disk: D, selector: S) -> io::Result<TCPProtocol>
        where D: LayerRegistration<ODiskMessage, IDiskMessage, SS2 = ActiveDiskManager> + 'static + Send,
              S: LayerRegistration<OSelectorMessage, OProtocolMessage> + 'static + Send
    {
        let mut config = Config::new();
        config.slab_capacity(MAX_CONNECTED_PEERS);
        // TODO: Figure our how rotor uses mio notify and timer capacities internally and set those

        let context = ProtocolContext::new(disk, selector);
        let mut eloop: Loop<AcceptPeer<TcpStream, Stream<PeerConnection>>> = try!(Loop::new(&config));

        let mut s_noti = None;
        eloop.add_machine_with(|early| {
            s_noti = Some(early.notifier());

            Response::ok(AcceptPeer::Shutdown)
        });

        let (p_send, p_recv) = mpsc::sync_channel(MAX_PENDING_NEW_PEERS);
        let mut p_noti = None;
        eloop.add_machine_with(|early| {
            p_noti = Some(early.notifier());

            Response::ok(AcceptPeer::Incoming(p_recv))
        });

        thread::spawn(move || eloop.run(context).expect("bip_peer: TCPProtocol Thread Shutdown Unexpectedly"));

        Ok(TCPProtocol {
            shutdown: s_noti.unwrap(),
            peer_send: PeerSender::new(p_send, p_noti.unwrap()),
        })
    }

    pub fn peer_sender(&self) -> Box<Sender<BTPeer>> {
        Box::new(self.peer_send.clone())
    }
}

impl Drop for TCPProtocol {
    fn drop(&mut self) {
        self.shutdown.wakeup();
    }
}

// ----------------------------------------------------------------------------//

#[derive(Clone)]
struct PeerSender {
    send: SyncSender<(TcpStream, PeerId, InfoHash)>,
    noti: Notifier,
}

impl PeerSender {
    fn new(send: SyncSender<(TcpStream, PeerId, InfoHash)>, noti: Notifier) -> PeerSender {
        PeerSender {
            send: send,
            noti: noti,
        }
    }
}

impl Sender<BTPeer> for PeerSender {
    fn send(&self, data: BTPeer) {
        let (stream, id, hash) = data.destroy();

        self.send
            .send((stream, id, hash))
            .expect("bip_peer: PeerSender Failed To Send Peer");
    }
}
