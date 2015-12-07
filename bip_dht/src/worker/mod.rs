use std::convert::{AsRef};
use std::io::{self};
use std::net::{SocketAddr, UdpSocket};

use bip_util::{InfoHash};
use mio::{Sender};

use router::{Router};
use routing::node::{Node};
use routing::table::{self, RoutingTable};

pub mod handler;
pub mod lookup;
pub mod messenger;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum OneshotTask {
    /// Process an incoming message from a remote node.
    Incoming(Vec<u8>, SocketAddr),
    /// Schedule an ScheduledTask to occur some time later.
    ScheduleTask(u64, ScheduledTask),
    /// Load a new bootstrap operation into worker storage.
    StartBootstrap(Vec<Router>, Vec<SocketAddr>),
    /// Start a lookup for the given InfoHash.
    StartLookup(InfoHash, SyncSender<()>)
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
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

pub fn start_mainline_dht<H>(send_socket: UdpSocket, recv_socket: UdpSocket, read_only: bool, ext_addr: Option<SocketAddr>,
    handshaker: H) -> io::Result<Sender<OneshotTask>> where H: Handshaker + 'static {
    let outgoing = messenger::create_outgoing_messenger(send_socket);

    let routing_table = RoutingTable::new(table::random_node_id());
    let message_sender = try!(handler::create_dht_handler(routing_table, outgoing, handshaker));

    let incoming = messenger::create_incoming_messenger(recv_socket, message_sender.clone());

    println!("TABLE NODE ID IS {:?}", routing_table.node_id());

    Ok(message_sender)
}