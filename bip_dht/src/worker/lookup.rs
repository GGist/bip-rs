use std::collections::{HashMap, HashSet};
use std::net::{SocketAddrV4, SocketAddr};
use std::sync::mpsc::{SyncSender};

use bip_util::{NodeId, InfoHash};
use mio::{EventLoop, Timeout};

use message::compact_info::{CompactNodeInfo, CompactValueInfo};
use message::get_peers::{GetPeersRequest, CompactInfoType, GetPeersResponse};
use routing::node::{Node};
use routing::table::{self, RoutingTable, ClosestNodes};
use worker::{IntervalTask};
use worker::handler::{DhtHandler};

const LOOKUP_TIMEOUT_MS: u64 = 600;

// Values set here correspond to the "Aggressive Lookup" variant
// Alpha value
const MAX_ACTIVE_SEARCHES: usize = 4;
// Beta value
//const MAX_RESPONSE_SEARCHES: usize = 3;

// Number of nodes we store to announce to at the end of a lookup
const NUM_ANNOUNCE_NODES:  usize = 8;

// TODO: Because I made the routing table iterators so damn useful, it is
// very tempting to use another routing table for lookups. We should see in
// the future if this is really the way to go but for now it basically has
// all of the characteristics we want in an iterative lookup data structure.

// Instead of having a filter over the routing tables closest nodes so that
// we dont send requests to already pinged nodes, just force the nodes to be
// marked as bad so they are eligible to be replaced by new nodes. Of course,
// doing that means we have to track the closest nodes in a separate array.

enum LookupProgress {
    Working,
    Values(Vec<(NodeId, SocketAddrV4)>),
    Completed
}

enum ILookupProgress {
    // Number of responses received for the current search
    Searching(usize),
    Announcing,
    Completed
}

struct HashLookup {
    self_id:         NodeId,
    target_id:       NodeId,
    progress:        ILookupProgress,
    // Distance that last closest node was
    last_closest:    NodeId,
    // (Distance From Target, Node)
    closest_nodes:   Vec<(NodeId, Node)>,
    requested_nodes: HashSet<Node>,
    active_searches: HashMap<Node, Timeout>,
    announce_tokens: HashMap<Node, Vec<u8>>
}

impl HashLookup {
    pub fn new(self_id: NodeId, target_id: NodeId, table: &RoutingTable, out: SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler>) -> Option<HashLookup> {
        // Populate closest nodes with alpha closest nodes from the routing table
        let mut closest_nodes = Vec::with_capacity(MAX_ACTIVE_SEARCHES);
        for node in table.closest_nodes(target_it).take(MAX_ACTIVE_SEARCHES) {
            insert_sorted_node(target_id, &mut closest_nodes, node.clone());
        }
        
        // Should be as far away as possible, therefore, all of the bits were set as if
        // all of the bits were different from the result of a xor with the target id
        let last_closest = [255u8; hash::NODE_ID_LEN].into();
        let mut hash_lookup = HashLookup{ self_id: self_id, target_id: target_id, progress: ILookupProgress::Searching(0),
            last_closest: last_closest, closest_nodes: closest_nodes, requested_nodes: HashSet::with_capacity(MAX_ACTIVE_SEARCHES),
            active_searches: HashMap::with_capacity(MAX_ACTIVE_SEARCHES), announce_tokens: HashMap::new() }
        
        // Initiate the first round of requests
        if hash_lookup.start_request_round(out, event_loop) {
           Some(hash_lookup)
        } else {
           None
        }
    }
    
    pub fn node_timeout(&mut self, node: Node, out: SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventHandler<DhtHandler>) -> LookupProgress {
		// Check if we are still in the search phase
        let responses = if let Some(ref mut r) = self.responses {
            // Increment number of responses received
            *r += 1;
            *r
        } else {
            return LookupProgress::Completed
        };
        
        // Remove the timeout value from the active searches
        self.active_searches.remove(&node);
        
        // Check if we need to start a new round of requests
        if responses == MAX_ACTIVE_SEARCHES {
            if !self.start_request_round(out, event_handler) {
                // If the next round of requests failed to start, we are now announcing
                // and so we will not be entering this method again, essentially the
                // search is over (or will be over)
                self.responses = None;
            }
        }
        
        LookupProgress::Working
    }
    
    pub fn node_response<'a>(&mut self, node: Node, msg: GetPeersResponse<'a>, out: SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventHandler<DhtHandler>) -> LookupProgress {
        // 
    }
    
    pub fn bulk_timeout(&mut self, out: SyncSencer<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>)
        -> LookupProgress {
        
    }
    
    fn start_request_round(&mut self, out: SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>) -> bool {
        let get_peers = GetPeersRequest::new(b"1", self.self_id.as_ref(), self.target_id.as_ref()).unwrap();
        let get_peers_message = get_peers.encode();
        
        // Check if we got a closer id since the last round
        let closest_id = self.closest_nodes[0].0;
        let closest_id_dist = self.target_id ^ closest_id;
        
        if closest_id_dist < self.last_closest {
            self.last_closest = closest_id_dist;
            
            // Send messages to alpha number of closest nodes
            let mut messages_sent = 0;
            for node in self.
        } else {
            
        }
    }
}

    /// Start a new round of requests.
    ///
    /// Returns false if we failed to start a new round of requests, else returns true.
    fn start_request_round(&mut self, out: SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>) -> bool {
        let target_id = self.routing_table.node_id();
        
        let get_peers = GetPeersRequest::new(b"1", self.self_id.as_ref(), target_id.as_ref()).unwrap();
        let get_peers_message = get_peers.encode();
        
        // Check if we got a closer id since the last round
        let closest_node = self.routing_table.closest_nodes(target_id).next().unwrap();
        let current_closest = table::leading_bit_count(target_id, closest_node.id());
        if current_closest > self.last_closest {
            self.last_closest = current_closest;
            
            // Send messages to alpha number of closest nodes
            let mut messages_sent = 0;
            for node in self.routing_table.closest_nodes(target_id).filter(|n| !self.nodes_requested.contains(n) ).take(MAX_ACTIVE_SEARCHES) {
                // Send a message to the node
                if out.send((get_peers_message.clone(), node.addr())).is_err() {
                    error!("bip_dht: Could not send lookup message to node...");
                }
                
                // Mark the node as having been requested from
                self.nodes_requested.insert(node.clone());
                
                // Set a timeout for the request
                let timeout = event_loop.timeout_ms((LOOKUP_TIMEOUT_MS, IntervalTask::CheckNodeLookup(0, node.clone())), LOOKUP_TIMEOUT_MS);
                if let Ok(t) = timeout {
                    // Associate the timeout token with the current node
                    self.active_searches.insert(node.clone(), t);
                } else {
                    error!("bip_dht: Could not set timer for info hash lookup...");
                    return false
                }
                
                // Increment the number of messages sent
                // TODO: If the previous messages failed to get sent, this variable is a LIE
                messages_sent += 1;
            }
            
            messages_sent == MAX_ACTIVE_SEARCHES
        } else {
            // Should announce to the nodes if we wanted to announce
            false
        }
    }

/// Inserts the Node into the list of nodes based on its distance from the target node.
///
/// Nodes at the start of the list are closer to the target node than nodes at the end.
fn insert_sorted_node(target: NodeId, nodes: &mut Vec<(NodeId, Node)>, node: Node) {
    let node_dist = target ^ node.id();
    nodes.binary_search_by()
    let opt_position = nodes.iter().position(|(n_dist, _)| {
        node_dist < n_dist
    });
    
    match opt_position {
        Some(pos) => nodes.insert(pos, (node_dist, node);,
        None      => nodes.push((node_dist, node);
    };
}

struct HashLookup {
    self_id:         NodeId,
    // If set to Some, we are still in the lookup phase, if set to None
    // we have already sent bulk queries to the last remaining nodes and
    // have set the corresponding timeout.
    responses:       Option<usize>,
    last_closest:    usize,
    routing_table:   RoutingTable,
    active_searches: HashMap<Node, Timeout>,
    // Used as a filter for nodes we already requested from.
    nodes_requested: HashSet<Node>,
    // Can not be used as a filter for nodes we have already queried
    // since not all nodes may have responded to our query!
    announce_tokens: HashMap<Node, Vec<u8>>
}

impl HashLookup {
    /// Create a new HashLookup to lookup the given id seeded with the given closest nodes.
    pub fn new<'a>(self_id: NodeId, lookup_hash: InfoHash, closest_nodes: ClosestNodes<'a>,
        out: SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>) -> Option<HashLookup> {
        let mut routing_table = RoutingTable::new(lookup_hash);
        
        // Add alpha number of closest nodes to the lookup routing table
        for node in closest_nodes.take(MAX_ACTIVE_SEARCHES) {
            routing_table.add_node(node.clone());
        }
        
        let mut hash_lookup = HashLookup{ self_id: self_id, responses: Some(0), last_closest: 0, routing_table: routing_table,
            active_searches: HashMap::new(), nodes_requested: HashSet::new(), announce_tokens: HashMap::new() };
        
        if hash_lookup.start_request_round(out, event_loop) {
            Some(hash_lookup)
        } else {
            None
        }
    }
    
    pub fn node_timeout(&mut self, out: SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>,
        node: Node) -> LookupProgress {
        // Increment the number of responses otherwise, we are not in the search phase anymore
        let responses = if let Some(ref mut responses) = self.responses {
            *responses += 1;
            *responses
        } else {
            return LookupProgress::Completed
        };
        // We are still in the search phase
        
        // Remove the timeout value from the active searches
        self.active_searches.remove(&node);
        
        // Check if we have received all the responses for the round
        if responses == MAX_ACTIVE_SEARCHES {
            // Check if we failed to start a new request round
            if !self.start_request_round(out, event_loop) {
                // Proceed to announce (if thats what was requested)
                
                return LookupProgress::Completed
            }
        }
        
        LookupProgress::Working
    }
    
    pub fn response<'a>(&mut self, out: SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>,
        node: Node, response: GetPeersResponse<'a>) -> LookupProgress {
        // Increment the number of responses otherwise, we are not in the search phase anymore
        let responses = if let Some(ref mut responses) = self.responses {
            *responses += 1;
            *responses
        } else {
            return LookupProgress::Completed
        };
        // We are still in the search phase
            
        // Clear and remove the timeout value
        if let Some(timeout) = self.active_searches.remove(&node) {
            event_loop.clear_timeout(timeout);
        }
        
        // Add the announce token to our map
        if let Some(token) = response.token() {
            self.announce_tokens.insert(node.clone(), token.to_vec());
        }
        
        // Check if we got peers, values, or both
        let values = match response.info_type() {
            CompactInfoType::Nodes(n) => {
                for (id, v4_addr) in n {
                    let addr = SocketAddr::V4(v4_addr);
                    let node = Node::as_questionable(id, addr);
                    
                    self.routing_table.add_node(node);
                }
                None
            },
            CompactInfoType::Values(v) => {
                Some(v.into_iter().collect())
            },
            CompactInfoType::Both(n, v) => {
                for (id, v4_addr) in n {
                    let addr = SocketAddr::V4(v4_addr);
                    let node = Node::as_questionable(id, addr);
                    
                    self.routing_table.add_node(node);
                }
                Some(v.into_iter().collect())
            }
        };
        
        // Check if we have received all the responses for the round
        if responses == MAX_ACTIVE_SEARCHES {
            // Check if we failed to start a new request round
            if !self.start_request_round(out, event_loop) {
                // Proceed to announce (if thats what was requested)
                
                if let Some(values) = values {
                    return LookupProgress::Values(values)
                } else {
                    return LookupProgress::Completed
                }
            }
        }
        
        if let Some(values) = values {
            LookupProgress::Values(values)
        } else {
            LookupProgress::Working
        }
    }
    
    pub fn bulk_timeout(&mut self, out: SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>) -> LookupProgress {
        LookupProgress::Completed
    }
    
    /// Start a new round of requests.
    ///
    /// Returns false if we failed to start a new round of requests, else returns true.
    fn start_request_round(&mut self, out: SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>) -> bool {
        let target_id = self.routing_table.node_id();
        
        let get_peers = GetPeersRequest::new(b"1", self.self_id.as_ref(), target_id.as_ref()).unwrap();
        let get_peers_message = get_peers.encode();
        
        // Check if we got a closer id since the last round
        let closest_node = self.routing_table.closest_nodes(target_id).next().unwrap();
        let current_closest = table::leading_bit_count(target_id, closest_node.id());
        if current_closest > self.last_closest {
            self.last_closest = current_closest;
            
            // Send messages to alpha number of closest nodes
            let mut messages_sent = 0;
            for node in self.routing_table.closest_nodes(target_id).filter(|n| !self.nodes_requested.contains(n) ).take(MAX_ACTIVE_SEARCHES) {
                // Send a message to the node
                if out.send((get_peers_message.clone(), node.addr())).is_err() {
                    error!("bip_dht: Could not send lookup message to node...");
                }
                
                // Mark the node as having been requested from
                self.nodes_requested.insert(node.clone());
                
                // Set a timeout for the request
                let timeout = event_loop.timeout_ms((LOOKUP_TIMEOUT_MS, IntervalTask::CheckNodeLookup(0, node.clone())), LOOKUP_TIMEOUT_MS);
                if let Ok(t) = timeout {
                    // Associate the timeout token with the current node
                    self.active_searches.insert(node.clone(), t);
                } else {
                    error!("bip_dht: Could not set timer for info hash lookup...");
                    return false
                }
                
                // Increment the number of messages sent
                // TODO: If the previous messages failed to get sent, this variable is a LIE
                messages_sent += 1;
            }
            
            messages_sent == MAX_ACTIVE_SEARCHES
        } else {
            // Should announce to the nodes if we wanted to announce
            false
        }
    }
}