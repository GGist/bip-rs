use std::convert::{AsRef};
use std::io::{self};
use std::net::{SocketAddr, UdpSocket};

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
    /// Schedule an IntervalTask to occur some time later.
    ScheduleTask(u64, IntervalTask),
    /// Load a new bootstrap operation into worker storage.
    StartBootstrap(Vec<Router>, Vec<SocketAddr>)
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum IntervalTask {
    /// Check the progress of the bucket refresh.
    CheckRefresh(usize),
    /// Check the progress of the current bootstrap.
    CheckBootstrap(usize),
    /// Check the progress of a current lookup.
    /// (Index Of Current Lookup, Index Of Message)
    CheckNodeLookup(usize, Node),
    /// Check the progress of the lookup endgame.
    CheckBulkLookup(usize)
}

pub fn start_mainline_dht(send_socket: UdpSocket, recv_socket: UdpSocket, read_only: bool,
    ext_addr: Option<SocketAddr>) -> io::Result<Sender<OneshotTask>> {
    let outgoing = messenger::create_outgoing_messenger(send_socket);

    let routing_table = RoutingTable::new(table::random_node_id());
    println!("TABLE NODE ID IS {:?}", routing_table.node_id());
    let message_sender = try!(handler::create_dht_handler(routing_table, outgoing));

    let incoming = messenger::create_incoming_messenger(recv_socket, message_sender.clone());

    Ok(message_sender)
}