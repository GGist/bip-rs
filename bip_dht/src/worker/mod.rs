use std::io::{self};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{self};

use bip_handshake::{Handshaker};
use bip_util::bt::{InfoHash};
use mio::{self};

use router::{Router};
use routing::table::{self, RoutingTable};
use transaction::{TransactionID};

pub mod bootstrap;
pub mod handler;
pub mod lookup;
pub mod messenger;
pub mod refresh;

/// Task that our DHT will execute immediately.
#[derive(Clone)]
pub enum OneshotTask {
    /// Process an incoming message from a remote node.
    Incoming(Vec<u8>, SocketAddr),
    /// Register a sender to send DhtEvents to.
    RegisterSender(mpsc::Sender<DhtEvent>),
    /// Load a new bootstrap operation into worker storage.
    StartBootstrap(Vec<Router>, Vec<SocketAddr>),
    /// Start a lookup for the given InfoHash.
    StartLookup(InfoHash, bool),
    /// Gracefully shutdown the DHT and associated workers.
    Shutdown(ShutdownCause)
}

/// Task that our DHT will execute some time later.
#[derive(Copy, Clone, Debug)]
pub enum ScheduledTask {
    /// Check the progress of the bucket refresh.
    CheckTableRefresh(TransactionID),
    /// Check the progress of the current bootstrap.
    CheckBootstrapTimeout(TransactionID),
    /// Check the progress of a current lookup.
    CheckLookupTimeout(TransactionID),
    /// Check the progress of the lookup endgame.
    CheckLookupEndGame(TransactionID)
}

/// Event that occured within the DHT which clients may be interested in.
#[derive(Copy, Clone, Debug)]
pub enum DhtEvent {
    /// DHT completed the bootstrap.
    BootstrapCompleted,
    /// Lookup operation for the given InfoHash completed.
    LookupCompleted(InfoHash),
    /// DHT is shutting down for some reason.
    ShuttingDown(ShutdownCause)
}

/// Event that occured within the DHT which caused it to shutdown.
#[derive(Copy, Clone, Debug)]
pub enum ShutdownCause {
    /// DHT failed to bootstrap more than once.
    BootstrapFailed,
    /// Client controlling the DHT intentionally shut it down.
    ClientInitiated,
    /// Cause of shutdown is not specified.
    Unspecified
}

/// Spawns the necessary workers that make up our local DHT node and connects them via channels
/// so that they can send and receive DHT messages.
pub fn start_mainline_dht<H>(send_socket: UdpSocket, recv_socket: UdpSocket, read_only: bool,
    _: Option<SocketAddr>, handshaker: H, kill_sock: UdpSocket, kill_addr: SocketAddr)
    -> io::Result<mio::Sender<OneshotTask>> where H: Handshaker + 'static {
    let outgoing = messenger::create_outgoing_messenger(send_socket);

    // TODO: Utilize the security extension.
    let routing_table = RoutingTable::new(table::random_node_id());
    let message_sender = try!(handler::create_dht_handler(routing_table, outgoing, read_only, handshaker,
        kill_sock, kill_addr));

    messenger::create_incoming_messenger(recv_socket, message_sender.clone());

    Ok(message_sender)
}