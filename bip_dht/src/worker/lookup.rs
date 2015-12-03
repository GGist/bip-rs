use std::collections::{HashMap, HashSet};
use std::net::{SocketAddrV4, SocketAddr};
use std::sync::mpsc::{SyncSender};

use bip_util::{self, NodeId, InfoHash};
use mio::{EventLoop, Timeout};

use message::compact_info::{CompactNodeInfo, CompactValueInfo};
use message::get_peers::{GetPeersRequest, CompactInfoType, GetPeersResponse};
use routing::node::{Node, NodeStatus};
use routing::table::{self, RoutingTable, ClosestNodes};
use worker::{IntervalTask};
use worker::handler::{DhtHandler};

const LOOKUP_TIMEOUT_MS: u64  = 600;
const ENDGAME_TIMEOUT_MS: u64 = 1500;

// TODO: Values set here correspond to the "Aggressive Lookup" variant
// Alpha value
const MAX_ACTIVE_SEARCHES: usize = 4;
// Beta value
//const MAX_RESPONSE_SEARCHES: usize = 3;

// Number of nodes we store to announce to at the end of a lookup
const NUM_ANNOUNCE_NODES:  usize = 8;

// Returned by our lookup structure to give information on a per request/timeout basis.
#[derive(Debug)]
pub enum LookupResult {
    Working,
    Values(Vec<SocketAddrV4>),
    Completed
}

// Stored by our lookup structure to track what phase of the lookup we are on.
enum LookupProgress {
    // Number of responses received for the current search
    Searching(usize),
    EndGame,
    Completed
}

// Returned by the internal request dispatcher for the status of the round.
enum RoundResult {
    Started,
    Completed,
    Failed
}

pub struct HashLookup {
    self_id:         NodeId,
    target_id:       NodeId,
    progress:        LookupProgress,
    // Distance that last closest node was
    last_closest:    NodeId,
    // (Distance From Target, Node, Pinged Yet)
    closest_nodes:   Vec<(NodeId, Node, bool)>,
    active_searches: HashMap<Node, Timeout>,
    announce_tokens: HashMap<Node, Vec<u8>>
}

impl HashLookup {
    pub fn new(self_id: NodeId, target_id: NodeId, table: &RoutingTable, out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler>) -> Option<HashLookup> {
        println!("TARGET ID {:?}", target_id);
        // Populate closest nodes with alpha closest nodes from the routing table
        let mut closest_nodes = Vec::with_capacity(MAX_ACTIVE_SEARCHES);
        for node in table.closest_nodes(target_id).filter(|n| n.status() == NodeStatus::Good).take(MAX_ACTIVE_SEARCHES) {
            insert_sorted_node(target_id, &mut closest_nodes, node.clone());
        }
        
        // Should be as far away as possible, therefore, all of the bits were set as if
        // all of the bits were different from the result of a xor with the target id
        let last_closest = [255u8; bip_util::NODE_ID_LEN].into();
        let mut hash_lookup = HashLookup{ self_id: self_id, target_id: target_id,
            progress: LookupProgress::Searching(0), last_closest: last_closest, closest_nodes: closest_nodes,
            active_searches: HashMap::with_capacity(MAX_ACTIVE_SEARCHES), announce_tokens: HashMap::new() };
        
        // Initiate the first round of requests
        match hash_lookup.start_request_round(out, event_loop) {
            RoundResult::Started   => Some(hash_lookup),
            RoundResult::Completed => None,
            RoundResult::Failed    => None
        }
    }
    
    pub fn node_timeout(&mut self, node: Node, out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler>) -> LookupResult {
		// Check if we are still in the search phase
        let responses = if let LookupProgress::Searching(ref mut r) = self.progress {
            // Increment number of responses received
            *r += 1;
            *r
        } else {
            return LookupResult::Completed
        };
        
        // Remove the timeout value from the active searches
        self.active_searches.remove(&node);
        
        // Check if we need to start a new round of requests
        if responses == MAX_ACTIVE_SEARCHES {
            // Attempt to start a new round of requests
            self.progress = match self.start_request_round(out, event_loop) {
                RoundResult::Started   => LookupProgress::Searching(0),
                RoundResult::Completed => {
                    // Begin the endgame progress
                    self.start_endgame_round(out, event_loop);
                    
                    LookupProgress::EndGame
                },
                RoundResult::Failed => LookupProgress::Completed
            }
        }
        
        // Map the current progress to the user received progress
        match self.progress {
            LookupProgress::Searching(_) => LookupResult::Working,
            LookupProgress::EndGame      => LookupResult::Working,
            LookupProgress::Completed    => LookupResult::Completed
        }
    }
    
    pub fn node_response<'a>(&mut self, node: Node, msg: GetPeersResponse<'a>, out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler>) -> LookupResult {
        // Check if we are still in the search phase (still process stuff if we are in the end game phase though)
        let responses = match self.progress {
            LookupProgress::Searching(ref mut s) => {
                // Housekeeping unique to a regular search response but doesnt occur in an end game response
                
                // Increment the number of responses received
                *s += 1;
                
                // Remove the timeout token for the current search
                if let Some(timeout) = self.active_searches.remove(&node) {
                    event_loop.clear_timeout(timeout);
                }
                
                *s
            },
            LookupProgress::EndGame   => 0,
            LookupProgress::Completed => {
                return LookupResult::Completed
            }
        };
        
        // Store and associate the announce token with the node
        if let Some(token) = msg.token() {
            self.announce_tokens.insert(node.clone(), token.to_vec());
        }
        
        // Check if we got peers, values, or both
        let values = match msg.info_type() {
            CompactInfoType::Nodes(n) => {
                for (id, v4_addr) in n {
                    let addr = SocketAddr::V4(v4_addr);
                    let node = Node::as_good(id, addr);
                    
                    insert_sorted_node(self.target_id, &mut self.closest_nodes, node);
                }
                None
            },
            CompactInfoType::Values(v) => {
                Some(v.into_iter().collect())
            },
            CompactInfoType::Both(n, v) => {
                for (id, v4_addr) in n {
                    let addr = SocketAddr::V4(v4_addr);
                    let node = Node::as_good(id, addr);
                    
                    insert_sorted_node(self.target_id, &mut self.closest_nodes, node);
                }
                Some(v.into_iter().collect())
            }
        };
        
        // Check if we need to start a new round of requests
        if responses == MAX_ACTIVE_SEARCHES {
            self.progress = match self.start_request_round(out, event_loop) {
                RoundResult::Started   => LookupProgress::Searching(0),
                RoundResult::Completed => {
                    // Start the endgame process
                    self.start_endgame_round(out, event_loop);
                    
                    LookupProgress::EndGame
                },
                RoundResult::Failed    => LookupProgress::Completed
            };
        }
        
        // Determine return value
        if let Some(values) = values {
            LookupResult::Values(values)
        } else {
            // Derive return value from current progress
            match self.progress {
                LookupProgress::Searching(_) => LookupResult::Working,
                LookupProgress::EndGame      => LookupResult::Working,
                LookupProgress::Completed    => LookupResult::Completed
            }
        }
    }
    
    pub fn bulk_timeout(&mut self, out: &SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>)
        -> LookupResult {
        // Proceed to announce!!! (If we wanted to...)
        //unimplemented!();
        LookupResult::Completed
    }
    
    /// Start a new round of peer gathering requests.
    ///
    /// Returns true if the round was successfully started or false if it failed to start.
    fn start_request_round(&mut self, out: &SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>) -> RoundResult {
        let get_peers = GetPeersRequest::new(b"1", self.self_id.as_ref(), self.target_id.as_ref()).unwrap();
        let get_peers_message = get_peers.encode();
        
        // Get the new closest distance
        let closest_id = self.closest_nodes[0].0;
        let closest_id_dist = self.target_id ^ closest_id;
        
        // Check if distance is closer since the last round
        if closest_id_dist < self.last_closest {
            self.last_closest = closest_id_dist;
            println!("FOUND CLOSER ID {:?}", self.last_closest);
            // Send messages to alpha number of closest nodes
            let mut messages_sent = 0;
            for &mut (_, ref node, ref mut req) in self.closest_nodes.iter_mut().filter(|&&mut (_, _, req)| !req).take(MAX_ACTIVE_SEARCHES) {
                // Send a message to the node
                if out.send((get_peers_message.clone(), node.addr())).is_err() {
                    error!("bip_dht: Could not send lookup message to node...");
                }
                
                // Mark the node as having been requested from
                *req = true;
                
                // Set a timeout for the request
                let timeout = event_loop.timeout_ms((LOOKUP_TIMEOUT_MS, IntervalTask::CheckNodeLookup(0, node.clone())), LOOKUP_TIMEOUT_MS);
                if let Ok(t) = timeout {
                    // Associated the timeout token with the current node
                    self.active_searches.insert(node.clone(), t);
                } else {
                    error!("bip_dht: Could not set a timeout for info hash lookup...");
                    return RoundResult::Failed
                }
                
                // Increment the number of messages sent
                // TODO: THIS IS A LIE
                messages_sent += 1;
            }
            
            if messages_sent == MAX_ACTIVE_SEARCHES {
                RoundResult::Started
            } else {
                // Will hit this if we dont have enough nodes to request from,
                // in this case, we can start the end game but will realize again
                // that we dont have any nodes to process, therefore, we will announce
                RoundResult::Completed
            }
        } else {
            // Should begin the end game process
            RoundResult::Completed
        }
    }
    
    fn start_endgame_round(&mut self, out: &SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>) -> RoundResult {
        let get_peers = GetPeersRequest::new(b"1", self.self_id.as_ref(), self.target_id.as_ref()).unwrap();
        let get_peers_message = get_peers.encode();
        
        // Attempt to start a timeout for an endgame
        let timeout = event_loop.timeout_ms((ENDGAME_TIMEOUT_MS, IntervalTask::CheckBulkLookup(0)), ENDGAME_TIMEOUT_MS);
        if timeout.is_err() {
            error!("bip_dht: Could not set a timeout for the info hash lookup end game...");
            return RoundResult::Failed
        }
        
        // Send messages to all unpinged nodes
        for &mut (_, ref node, ref mut req) in self.closest_nodes.iter_mut().filter(|&&mut (_, _, req)| !req) {
            // Should really need to update this...
            *req = true;
            
            // Send a message to the node
            if out.send((get_peers_message.clone(), node.addr())).is_err() {
                error!("bip_dht: Could not send a lookup message to a node...");
            }
        }
        
        RoundResult::Started
    }
}

/// Inserts the Node into the list of nodes based on its distance from the target node.
///
/// Nodes at the start of the list are closer to the target node than nodes at the end.
fn insert_sorted_node(target: NodeId, nodes: &mut Vec<(NodeId, Node, bool)>, node: Node) {
    let node_id = node.id();
    let node_dist = target ^ node_id;
    
    let search_result = nodes.binary_search_by(|&(dist, _, _)| dist.cmp(&node_dist));
    
    match search_result {
        Ok(dup_index) => {
            // Insert only if this node is different (it is ok if they have the same id)
            if nodes[dup_index].1 != node {
                nodes.insert(dup_index, (node_id, node, false));
            }
        },
        Err(ins_index) => nodes.insert(ins_index, (node_id, node, false))
    };
}