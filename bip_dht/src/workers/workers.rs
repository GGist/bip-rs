use std::collections::{HashMap};
use std::collections::hash_map::{Entry};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread::{self};
use std::time::{Duration};

use time::{PreciseTime};

use {InfoHash, NodeId};
use bencode::{Bencode};
use dht::{Router};
use dht::builder::{DhtBuilder};
use dht::message::{MessageType};
use dht::message::response::{ResponseType, ExpectedResponse};
use dht::message::find_node::{FindNodeRequest, FindNodeResponse};
use dht::message::get_peers::{GetPeersRequest, GetPeersResponse};
use dht::node::{Node};
use dht::security::{self};
use dht::table::{RoutingTable, BucketContents};
use dht::token::{Token};
use dht::transaction::{TransactionId};
use error::{DhtResult, DhtError, GenericError};
use handshake::{Handshaker};

pub fn start_dht<N, R, H>(handshaker: H, nodes: N, routers: R, send_socket: UdpSocket, recv_socket: UdpSocket,
    read_only: bool, ext_addr: Option<SocketAddr>) -> Sender<WorkerMessage> where N: IntoIterator<Item=SocketAddr>,
    R: IntoIterator<Item=Router>, H: Handshaker + 'static {
    let (out_send, out_recv) = mpsc::channel();
    let (in_send, in_recv) = mpsc::channel();
    
    spawn_sender(send_socket, out_recv);
    spawn_receiver(recv_socket, in_send);
    
    let (msg_send, msg_recv) = mpsc::channel();
    let node_id = get_node_id(ext_addr);
    let storage = WorkerStorage{ node_id: node_id, handshaker: handshaker, is_read_only: read_only,
        routing_table: RoutingTable::new(node_id), external_addr: ext_addr, active_searches: HashMap::new(),
        active_bootstraps: HashMap::new() };
    
    spawn_worker(storage, in_recv, out_send, msg_recv);
    spawn_supervisor(nodes.into_iter(), routers.into_iter(), msg_send.clone());
    
    msg_send
}

/// TODO: Replace this!!!
fn get_node_id(external_addr: Option<SocketAddr>) -> NodeId {
    let ip_addr = match external_addr.unwrap() {
        SocketAddr::V4(v4) => *v4.ip(),
        SocketAddr::V6(v6) => panic!("Unimplemented Ipv6 Node Id Generation Code")
    };
    
    security::generate_compliant_id_ipv4(ip_addr)
}

//----------------------------------------------------------------------------//

type MessageSender   = Sender<(SocketAddr, Vec<u8>)>;
type MessageReceiver = Receiver<(SocketAddr, Vec<u8>)>;

const MAX_BOOTSTRAP_ITERATIONS: u8 = 1;
const MAX_SEARCH_ITERATIONS:    u8 = 8;

/// Tracks the progress of a bootstrap operation.
/// Probably going to reuse this for refreshes for the time being,
/// eventually we may want to do less that 8 iterations for a refresh.
struct BootstrapProgress {
    target:        NodeId,
    /// Set to true if the initial node should not be added to routing table.
    from_router:   bool,
    last_updated:  PreciseTime,
    /// Set to true if a node id responds that isnt being tracked (initial message response).
    init_response: bool,
    visited_nodes: HashMap<NodeId, u8>
}

/// Tracks the progress of a search or announce operation.
struct SearchProgress {
    target:        InfoHash, 
    // Some if we are announcing, None if just searching
    closest:       Option<Vec<(NodeId, Token)>>,
    last_updated:  PreciseTime,
    visited_nodes: HashMap<NodeId, u8>
}

/// Persistent storage for a worker thread.
/// TODO: Not good to share transaction ids across a complete search since
/// nodes could spoof a node id of another node in that search (although if
/// they got it wrong it MAY still be counted as a 0th iteration...).
struct WorkerStorage<H> {
    node_id:           NodeId,
    handshaker:        H,
    is_read_only:      bool,
    routing_table:     RoutingTable,
    external_addr:     Option<SocketAddr>,
    active_searches:   HashMap<TransactionId, SearchProgress>,
    active_bootstraps: HashMap<TransactionId, BootstrapProgress>
}

/// Messages that can be sent to a worker thread.
pub enum WorkerMessage {
    /// Supervisor Messages
    RefreshBuckets,
    BootstrapNode(SocketAddr),
    BootstrapRouter(Router),
    
    /// User Messages
    Search(InfoHash),
    Announce(InfoHash)
}

fn spawn_worker<H>(mut storage: WorkerStorage<H>, in_recv: MessageReceiver, out_send: MessageSender,
    msg_recv: Receiver<WorkerMessage>,) where H: Handshaker + 'static {
    thread::spawn(move || {
        loop {
            select! {
                proc_msg = in_recv.recv() => {
                    handle_proc_message(&mut storage, proc_msg.unwrap(), &out_send)
                },
                node_msg = msg_recv.recv() => {
                    handle_node_message(&mut storage, node_msg.unwrap(), &out_send)
                }
            }
        }
    });
}

fn handle_proc_message<H>(storage: &mut WorkerStorage<H>, message: (SocketAddr, Vec<u8>), out_send: &MessageSender)
    where H: Handshaker {
    let bencode_message = match Bencode::decode(&message.1) {
        Ok(msg)  => msg,
        Err(_) => return
    };
    
    let krpc_message = MessageType::new(&bencode_message, |id| {
        let transaction_id = TransactionId::from_bytes(id).unwrap();
        
        if storage.active_searches.contains_key(&transaction_id) {
            ExpectedResponse::GetPeers
        } else if storage.active_bootstraps.contains_key(&transaction_id) {
            ExpectedResponse::FindNode
        } else {
            ExpectedResponse::None
        }
    }).unwrap();
    
    match krpc_message {
        MessageType::Response(ResponseType::FindNode(find_node)) => {
            handle_find_node(storage, find_node, message.0, out_send)
        },
        MessageType::Response(ResponseType::GetPeers(get_peers)) => {
            handle_get_peers(storage, get_peers, message.0, out_send)
        },
        _ => ()
    };
}

fn handle_find_node<'a, H>(storage: &mut WorkerStorage<H>, message: FindNodeResponse<'a>, addr: SocketAddr, out_send: &MessageSender)
    where H: Handshaker {
    let transaction_id = TransactionId::from_bytes(message.transaction_id()).unwrap();
    let bootstrap_progress = storage.active_bootstraps.get_mut(&transaction_id).unwrap();
    
    let remote_node_id = NodeId::from_bytes_be(message.node_id()).unwrap();
    
    let bootstrap_iteration = match bootstrap_progress.visited_nodes.entry(remote_node_id) {
        Entry::Occupied(occ) => *occ.get(),
        Entry::Vacant(vac)   => 0
    };
    
    let find_node = FindNodeRequest::new(message.transaction_id(), storage.node_id.as_bytes(),
            bootstrap_progress.target.as_bytes()).unwrap();
    
    // Check if this is an unsolicited response (possible attack vector)
    // TODO: Other possible attack vector, attacker node gives us a response after we started a search on another node but before that other node responds
    let unsolicited_iteration = bootstrap_iteration == 0 && bootstrap_progress.init_response;
    if bootstrap_iteration < MAX_BOOTSTRAP_ITERATIONS && !unsolicited_iteration {
        let find_node_bytes = find_node.encode();
        
        for (id, addr) in message.nodes() {
            bootstrap_progress.visited_nodes.insert(id, bootstrap_iteration + 1);
        
            let socket_addr = SocketAddr::V4(addr);
            out_send.send((socket_addr, find_node_bytes.clone())).unwrap();
        }
        // MOVE OUT OF IF statement so this triggers on last responses as well
        // Dont add routers to our routing table
        if bootstrap_iteration != 0 || !bootstrap_progress.from_router {
            let id = NodeId::from_bytes_be(find_node.node_id()).unwrap();
            storage.routing_table.add_node(Node::new(id, addr));
            
            for (index, bucket) in storage.routing_table.buckets().enumerate() {
                let nodes = match bucket {
                    BucketContents::Empty => 0,
                    BucketContents::Sorted(b) => b.iter().count(),
                    BucketContents::Assorted(b) => b.iter().count()
                };
                
                println!("Bucket {} Contains {} Nodes", index, nodes);
            }
        }
        
        bootstrap_progress.last_updated = PreciseTime::now();
        bootstrap_progress.init_response = true;
    }
}

fn handle_get_peers<'a, H>(storage: &mut WorkerStorage<H>, message: GetPeersResponse<'a>, addr: SocketAddr, out_send: &MessageSender)
    where H: Handshaker {
    let transaction_id = TransactionId::from_bytes(message.transaction_id()).unwrap();
    let search_progress = storage.active_searches.get_mut(&transaction_id).unwrap();
    
    
}

fn handle_node_message<H>(storage: &mut WorkerStorage<H>, message: WorkerMessage, out_send: &MessageSender)
    where H: Handshaker {
    match message {
        WorkerMessage::BootstrapNode(socket_addr) => handle_new_bootstrap(storage, socket_addr, false, out_send),
        WorkerMessage::BootstrapRouter(router)    => {
            let socket_addr = SocketAddr::V4(router.ipv4_addr().unwrap());
            
            handle_new_bootstrap(storage, socket_addr, true, out_send);
        },
        _ => ()
    };
}

fn handle_new_bootstrap<H>(storage: &mut WorkerStorage<H>, addr: SocketAddr, is_router: bool, out_send: &MessageSender)
    where H: Handshaker {
    let transaction_id = TransactionId::new();
    let bootstrap_progress = BootstrapProgress{ target: storage.node_id, from_router: is_router,
        last_updated: PreciseTime::now(), init_response: false, visited_nodes: HashMap::new() };
    
    storage.active_bootstraps.insert(transaction_id, bootstrap_progress);
    
    let find_node = FindNodeRequest::new(transaction_id.as_bytes(), storage.node_id.as_bytes(), storage.node_id.as_bytes()).unwrap();
    out_send.send((addr, find_node.encode())).unwrap();
}

fn spawn_supervisor<N, R>(nodes: N, routers: R, sender: Sender<WorkerMessage>)
    where N: Iterator<Item=SocketAddr>, R: Iterator<Item=Router> {
    // Bootstrap From Nodes
    for node in nodes {
        sender.send(WorkerMessage::BootstrapNode(node)).unwrap();
    }
        
    // Bootstrap From Routers
    for router in routers {
        sender.send(WorkerMessage::BootstrapRouter(router)).unwrap();
    }
    
    thread::spawn(move || {
        // Notify For Bucket Refreshes
        loop {
            let sleep_dur = Duration::from_secs(5 * 60);
            
            sender.send(WorkerMessage::RefreshBuckets).unwrap();
            thread::sleep(sleep_dur);
        }
    });
}

//----------------------------------------------------------------------------//

fn spawn_sender(socket: UdpSocket, receiver: MessageReceiver) {
    thread::spawn(move || {
        for (addr, buff) in receiver {
            socket.send_to(&buff, addr).unwrap();
        }
    });
}

fn spawn_receiver(socket: UdpSocket, sender: MessageSender) {
    thread::spawn(move || {
        loop {
            let mut buff = vec![0u8; 1500];
            
            let (size, addr) = socket.recv_from(&mut buff).unwrap();
            buff.truncate(size);
            
            sender.send((addr, buff)).unwrap();
        }
    });
}