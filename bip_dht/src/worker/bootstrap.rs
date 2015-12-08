use std::collections::{HashSet, HashMap};
use std::net::{SocketAddr};
use std::sync::mpsc::{SyncSender};

use bip_util::{self, NodeId};
use mio::{Timeout, EventLoop};

use message::find_node::{FindNodeResponse, FindNodeRequest};
use routing::node::{Node, NodeStatus};
use routing::table::{self, RoutingTable};
use transaction::{MIDGenerator, TransactionID};
use worker::{ScheduledTask};
use worker::handler::{DhtHandler};

const BOOTSTRAP_INITIAL_TIMEOUT: u64 = 1000;
const BOOTSTRAP_NODE_TIMEOUT:    u64 = 500;

const BOOTSTRAP_PINGS_PER_BUCKET: usize = 8;

#[derive(Debug)]
pub enum BootstrapStatus {
    Bootstrapping,
    Completed
}

pub struct TableBootstrap {
    table_id:              NodeId,
    id_generator:          MIDGenerator,
    starting_nodes:        Vec<SocketAddr>,
    active_messages:       HashMap<TransactionID, Timeout>,
    starting_routers:      HashSet<SocketAddr>,
    curr_refresh_bucket:   usize,
    curr_bootstrap_bucket: usize
}

impl TableBootstrap {
    pub fn new(table_id: NodeId, id_generator: MIDGenerator)
        -> TableBootstrap {
        TableBootstrap{ table_id: table_id, id_generator: id_generator, starting_nodes: Vec::new(),
            starting_routers: HashSet::new(), active_messages: HashMap::new(), curr_refresh_bucket: 0,
            curr_bootstrap_bucket: 0 }
    }
    
    pub fn start_bootstrap<H>(&mut self, nodes: Vec<SocketAddr>, routers: &[SocketAddr], table: &mut RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler<H>>) -> BootstrapStatus {
        // Reset the bootstrap state
        self.starting_nodes = nodes;
        self.starting_routers.clear();
        for router in routers.iter() {
            self.starting_routers.insert(*router);
        }
        self.active_messages.clear();
        self.curr_bootstrap_bucket = 0;
        
        // Generate transaction id for the initial bootstrap messages
        let trans_id = self.id_generator.generate();
        
        // Set a timer to begin the actual bootstrap
        let res_timeout = event_loop.timeout_ms((BOOTSTRAP_INITIAL_TIMEOUT, ScheduledTask::CheckBootstrapTimeout(trans_id)), BOOTSTRAP_INITIAL_TIMEOUT);
        let timeout = if let Ok(t) = res_timeout {
            t
        } else {
            error!("bip_dht: Failed to set a timeout for the start of a table bootstrap...");
            return self.finished_bootstrap()
        };
        
        // Insert the timeout into the active bootstraps just so we can check if a response was valid (and begin the bucket bootstraps)
        self.active_messages.insert(trans_id, timeout);
        
        let find_node_msg = FindNodeRequest::new(trans_id.as_ref(), self.table_id, self.table_id).encode();
        // Ping all initial routers and nodes
        for addr in self.starting_routers.iter().chain(self.starting_nodes.iter()) {
            if out.send((find_node_msg.clone(), *addr)).is_err() {
                error!("bip_dht: Failed to send bootstrap message to router through channel...");
            }
        }
        
        BootstrapStatus::Bootstrapping
    }
    
    pub fn is_router(&self, addr: &SocketAddr) -> bool {
        self.starting_routers.contains(&addr)
    }
    
    pub fn recv_response<'a, H>(&mut self, node: Node, trans_id: &TransactionID, msg: FindNodeResponse<'a>,
        table: &mut RoutingTable, out: &SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler<H>>)
        -> BootstrapStatus {
        // Process the message transaction id
        let timeout = if let Some(t) = self.active_messages.get(trans_id) {
            *t
        } else {
            warn!("bip_dht: Received unsolicited node response for an active table bootstrap...");
            return BootstrapStatus::Bootstrapping
        };
        
        // Add the given node as good in the routing table
        table.add_node(node);
        
        // Add the nodes from the response as questionable
        for (id, v4_addr) in msg.nodes() {
            let sock_addr = SocketAddr::V4(v4_addr);
            
            table.add_node(Node::as_questionable(id, sock_addr));
        }
        
        // If this response was from the initial bootstrap, we don't want to clear the timeout or remove
        // the token from the map as we want to wait until the proper timeout has been triggered before starting
        if self.curr_bootstrap_bucket != 0 {
            // Message was not from the initial ping
            // Remove the timeout from the event loop
            event_loop.clear_timeout(timeout);
            
            // Remove the token from the mapping
            self.active_messages.remove(trans_id);
        }
        
        // Check if we need to bootstrap on the next bucket
        if self.active_messages.is_empty() {
            if !self.bootstrap_next_bucket(table, out, event_loop) {
                return self.finished_bootstrap()
            }
        }
        
        BootstrapStatus::Bootstrapping
    }
    
    pub fn recv_timeout<H>(&mut self, trans_id: &TransactionID, table: &mut RoutingTable, out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>) -> BootstrapStatus {
        if self.active_messages.remove(trans_id).is_none() {
            warn!("bip_dht: Received unsolicited node timeout for an active table bootstrap...");
            return BootstrapStatus::Bootstrapping
        }
        
        // Check if we need to bootstrap on the next bucket
        if self.active_messages.is_empty() {
            if !self.bootstrap_next_bucket(table, out, event_loop) {
                return self.finished_bootstrap()
            }
        }
        
        BootstrapStatus::Bootstrapping
    }
    
    pub fn continue_refresh() -> bool {
        unimplemented!();
    }
    
    // Returns true if there are more buckets to bootstrap, false otherwise
    fn bootstrap_next_bucket<H>(&mut self, table: &RoutingTable, out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>) -> bool {
        let target_id = flip_id_bit_at_index(self.table_id, self.curr_refresh_bucket);
        
        let closest_good_nodes = table.closest_nodes(target_id).filter(|n| n.status() == NodeStatus::Good);
        let closest_questionable_nodes = table.closest_nodes(target_id).filter(|n| n.status() == NodeStatus::Questionable);
        
        let mut messages_sent = 0;
        for node in closest_questionable_nodes.chain(closest_good_nodes).take(BOOTSTRAP_PINGS_PER_BUCKET) {
            // Generate a transaction id
            let trans_id = self.id_generator.generate();
            let find_node_msg = FindNodeRequest::new(trans_id.as_ref(), self.table_id, target_id).encode();
            
            // Add a timeout for the node
            let res_timeout = event_loop.timeout_ms((BOOTSTRAP_NODE_TIMEOUT, ScheduledTask::CheckBootstrapTimeout(trans_id)), BOOTSTRAP_NODE_TIMEOUT);
            let timeout = if let Ok(t) = res_timeout {
                t
            } else {
                error!("bip_dht: Failed to set a timeout for the start of a table bootstrap...");
                return false
            };
            
            // Send the message to the node
            if out.send((find_node_msg, node.addr())).is_err() {
                error!("bip_dht: Could not send a bootstrap message through the channel...");
            }
            
            // Create an entry for the timeout in the map
            self.active_messages.insert(trans_id, timeout);
        }
        
        
        self.curr_bootstrap_bucket += 1;
        self.curr_bootstrap_bucket != table::MAX_BUCKETS
    }
    
    fn finished_bootstrap(&mut self) -> BootstrapStatus {
        self.active_messages.clear();
        
        BootstrapStatus::Completed
    }
}

/// Panics if index is out of bounds.
fn flip_id_bit_at_index(node_id: NodeId, index: usize) -> NodeId {
    let mut id_bytes: [u8; bip_util::NODE_ID_LEN]  = node_id.into();
    let (byte_index, bit_index) = (index / 8, index % 8);
    
    let actual_bit_index = 7 - bit_index;
    id_bytes[byte_index] ^= 1 << actual_bit_index;
    
    id_bytes.into()
}








/*

// To make bootstraps scalable, the number of bootstrap progresses that can be run in parallel
// should be a fraction of the number of discovered nodes for the current bootstrap process.
// This should prevent us from sending out too many requests to the same set of nodes in a small
// amount of time.

const BUCKET_BOOTSTRAP_BUCKET_SKIPS:         usize = 5;    // Buckets (bits) to skip per bootstrapped bucket
const BUCKET_BOOTSTRAP_PINGS_PER_BUCKET:     usize = 8;   // Nodes to ping per bootstrapped bucket
const BUCKET_BOOTSTRAP_TIMEOUT_MILLIS:       i64   = 5000; // Seconds to wait before declaring a request lost
const BUCKET_BOOTSTRAP_NODES_PER_BOOTSTRAP:  usize = 10;   // Ratio of nodes to parallel progresses run
const BUCKET_BOOTSTRAP_PARALLEL_REQUESTS:    usize = 8;    // Number of parallel requests per progress

/// Tracks information related to a routing table bootstrap process.
pub struct TableBootstrap {
    table_node_id:          NodeId,
    active_messages:      Vec<BucketBootstrap>,
    discovered_nodes:       Vec<SocketAddr>,
    discovered_routers:     HashSet<SocketAddr>,
    next_bucket_hash_index: usize,
    next_bucket_node_index: usize
}

enum BootstrapStatus {
    /// Indicates that there are no bootstraps left.
    NoBootstrapsLeft,
    /// Indicates that a new bootstrap can be started targeting the given NodeId and
    /// starting at the given index for discovered nodes.
    NextBootstrap(NodeId, usize),
    /// Indicates that the maximum number of concurrent bootstraps are being executed.
    MaxConcurrentBootstraps
}

impl TableBootstrap {
    /// Creates a new TableBootstrap that is targeting the given table id and is using the given routers.
    pub fn new(id: NodeId, routers: HashSet<SocketAddr>) -> TableBootstrap {
        TableBootstrap{ table_node_id: id, active_messages: Vec::new(), discovered_nodes: Vec::new(),
            discovered_routers: routers, next_bucket_hash_index: 0, next_bucket_node_index: 0 }
    }
    
    /// Returns true if the bootstrapping process has finished and false otherwise.
    pub fn check_bootstrap(&mut self, out: &SyncSender<(Vec<u8>, SocketAddr)>) -> bool {
        // Clear active bootstraps that are finished
        self.active_messages.retain( |bootstrap|
            !bootstrap.is_done()
        );
        
        // Check if we can start more bootstraps
        let mut available_bootstraps = true;
        let mut bootstrap_finished = false;
        while available_bootstraps {
            match self.bootstrap_status() {
                BootstrapStatus::NextBootstrap(id, index) => {
                    self.active_messages.push(BucketBootstrap::new(id, index));
                },
                BootstrapStatus::MaxConcurrentBootstraps => {
                    available_bootstraps = false;
                    bootstrap_finished = false;
                },
                BootstrapStatus::NoBootstrapsLeft => {
                    available_bootstraps = false;
                    bootstrap_finished = true;
                }
            };
        }
        
        // Run all active bootstraps
        for bucket_bootstrap in self.active_messages.iter_mut() {
            bucket_bootstrap.ping_nodes(&self.discovered_nodes[..], &self.table_node_id, out);
        }
        
        bootstrap_finished
    }
    
    /// Adds a node to the discovered nodes list for the current bootstrap.
    pub fn discovered_node(&mut self, node_addr: SocketAddr) {
        self.discovered_nodes.push(node_addr);
    }
    
    /// Returns true if the given address points to a router that is being used for bootstrapping.
    pub fn is_router(&self, router_addr: SocketAddr) -> bool {
        self.discovered_routers.contains(&router_addr)
    }
    
    /// Returns the current bootstrap status.
    fn bootstrap_status(&mut self) -> BootstrapStatus {
        let max_concurrent_bootstraps = self.discovered_nodes.len() / BUCKET_BOOTSTRAP_NODES_PER_BOOTSTRAP + 1;
    
        if self.next_bucket_hash_index >= table::MAX_BUCKETS {
            BootstrapStatus::NoBootstrapsLeft
        } else if self.active_messages.len() >= max_concurrent_bootstraps {
            BootstrapStatus::MaxConcurrentBootstraps
        } else {
            let id = flip_id_bit_at_index(self.table_node_id, self.next_bucket_hash_index);
            println!("Starting Bootstrap For Id {:?}", id);
            let index = self.next_bucket_node_index;
            
            self.next_bucket_node_index += BUCKET_BOOTSTRAP_PINGS_PER_BUCKET;
            self.next_bucket_hash_index += BUCKET_BOOTSTRAP_BUCKET_SKIPS + 1;
            
            BootstrapStatus::NextBootstrap(id, index)
        }
    }
}

/// Panics if index is out of bounds.
fn flip_id_bit_at_index(node_id: NodeId, index: usize) -> NodeId {
    let mut id_bytes: [u8; bip_util::NODE_ID_LEN]  = node_id.into();
    let (byte_index, bit_index) = (index / 8, index % 8);
    
    let actual_bit_index = 7 - bit_index;
    id_bytes[byte_index] ^= 1 << actual_bit_index;
    
    id_bytes.into()
}

//----------------------------------------------------------------------------//

/// Tracks information related to a bucket bootstrap process.
struct BucketBootstrap {
    target_id:    NodeId,
    next_index:   usize,
    pinged_nodes: usize
}

impl BucketBootstrap {
    /// Creates a new BucketBootstrap that is targeting the given id and is starting at the given index for pinging nodes.
    fn new(target_id: NodeId, start_index: usize) -> BucketBootstrap {
        BucketBootstrap{ target_id: target_id, next_index: start_index, pinged_nodes: 0 }
    }
    
    /// Pings the next round of nodes for the current bucket bootstrap.
    fn ping_nodes(&mut self, nodes: &[SocketAddr], self_id: &NodeId, out: &SyncSender<(Vec<u8>, SocketAddr)>) {
        if self.is_done() {
            return
        }
        let find_node = FindNodeRequest::new(&b"0"[..], self_id.as_bytes(), self.target_id.as_bytes()).unwrap();
        let find_node_bytes = find_node.encode();
    
        for node_addr in nodes.iter().cycle().skip(self.next_index).take(BUCKET_BOOTSTRAP_PARALLEL_REQUESTS) {
            if let Err(_) = out.send((find_node_bytes.clone(), *node_addr)) {
                warn!("bip_dht: Bucket bootstrap failed to send an outgoing bootstrap message...");
            }
            
            self.next_index += 1;
            self.pinged_nodes += 1;
        }
    }
    
    /// Returns true if the bucket bootstrap has finished.
    fn is_done(&self) -> bool {
        self.pinged_nodes >= BUCKET_BOOTSTRAP_PINGS_PER_BUCKET
    }
}*/