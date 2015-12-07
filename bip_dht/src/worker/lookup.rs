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

const LOOKUP_TIMEOUT_MS:  u64 = 1500;
const ENDGAME_TIMEOUT_MS: u64 = 1500;

// Currently using the aggressive variant of the standard lookup procedure.
// https://people.kth.se/~rauljc/p2p11/jimenez2011subsecond.pdf

// TODO: Handle case where a request round fails, should we fail the whole lookup (clear acvite lookups?)

const INITIAL_PICK_NUM:   usize = 4; // Alpha
const ITERATIVE_PICK_NUM: usize = 3; // Beta
const ANNOUNCE_PICK_NUM:  usize = 8; // # Announces

type Distance = ShaHash;

#[derive(Debug)]
pub enum LookupStatus {
    Searching,
    Values(Vec<SocketAddrV4>),
    Completed
}

pub struct TableLookup {
    table_id:        NodeId,
    target_id:       NodeId,
    in_endgame:      bool,
    id_generator:    MIDGenerator,
    closest_nodes:   Vec<(Distance, Node, bool)>,
    active_lookups:  HashMap<TransactionID, (Distance, Timeout)>,
    announce_tokens: HashMap<Node, Vec<u8>>
}

impl TableLookup {
    pub fn new(table_id: NodeId, target_id: NodeId, id_generator: MIDGenerator, table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>)
        -> Option<TableLookup> {
        let bucket_size = super::MAX_BUCKET_SIZE;
        
        // Pick a buckets worth of nodes and sort them
        let mut closest_nodes = Vec::with_capacity(bucket_size);
        for node in table.closest_nodes(target_id).filter(|n| n.status() == NodeStatus::Good).take(bucket_size) {
            insert_sorted_node(target_id, &mut closest_nodes, node.clone());
        }
        
        // Construct the lookup table structure
        let mut table_lookup = TableLookup{ table_id: table_id, target_id: target_id, in_endgame: false,
            id_generator: id_generator, closest_nodes: closest_nodes, announce_tokens: HashMap::new()
            active_lookups: HashMap::with_capacity(INITIAL_PICK_NUM) };
        
        // Ping alpha nodes that are closest to the id
        if table_lookup.start_request_round(INITIAL_PICK_NUM, out, event_loop) {
            Some(table_lookup)
        } else {
            None
        }
    }
    
    pub fn recv_response<'a>(&mut self, node: Node, trans_id: &TransactionID, msg: GetPeersResponse<'a>,
        out: &SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>) -> LookupStatus {
        // Process the message transaction id
        let (dist, timeout) = if let Some(lookup) = self.active_lookups.remove(trans_id) {
            lookup
        } else {
            warn!("bip_dht: Received unsolicited node response for an active table lookup...");
            return self.current_lookup_status()
        }
        
        // Cancel the timeout
        event_loop.clear_timeout(timeout);
        
        // Process the contents of the message
        let (opt_values, got_closer) = match msg.info_type() {
            CompactInfoType::Nodes(n) => {
                let mut got_closer = false;
            
                for (id, v4_addr) in n {
                    let addr = SocketAddr::V4(v4_addr)
                    let node = Node::as_good(id, addr);
                    
                    got_closer = got_closer || self.is_closer_node(dist, &node);
                    insert_sorted_node(self.target_id, &mut self.closest_nodes, node);
                }
                
                (None, got_closer)
            },
            CompactInfoType::Values(v) => {
                (Some(v.into_iter().collect()), false)
            },
            CompactInfoType::Both(n, v) => {
                let mut got_closer = false;
            
                for (id, v4_addr) in n {
                    let addr = SocketAddr::V4(v4_addr);
                    let node = Node::as_good(id, addr);
                    
                    got_closer = got_closer || self.is_closer_node(dist, &node);
                    insert_sorted_node(self.target_id, &mut self.closest_nodes, node);
                }
                
                (Some(v.into_iter().collect()), got_closer)
            }
        };
        
        // Add the announce token to our list of tokens
        if let Some(token) = msg.token() {
            self.announce_tokens.insert(node, token.to_vec());
        }
        
        // Check if we need to iterate (not in the endgame already)
        if !self.in_endgame {
            // If the node gave us a closer id than its own to the target id, continue the search
            if got_closer {
                self.start_request_round(ITERATIVE_PICK_NUM, out, event_loop);
            }
            
            // If there are not more active lookups, start the endgame
            if self.active_lookups.is_empty() {
                self.start_endgame_round(out, event_loop);
            }
        }
        
        match opt_values {
            Some(values) => LookupStatus::Values(values),
            None         => LookupStatus::Searching
        }
    }
    
    pub fn recv_timeout(&mut self, trans_id: &TransactionID, out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler>) -> LookupStatus {
        let (_, timeout) = if let Some(lookup) = self.active_lookups.remove(trans_id) {
            lookup
        } else {
            warn!("bip_dht: Received unsolicited node timeout for an active table lookup...");
            return self.current_lookup_status()
        }
        
        // Check if we need to iterate (not in the endgame already)
        if !self.in_endgame {
            // If there are not more active lookups, start the endgame
            if self.active_lookups.is_empty() {
                self.start_endgame_round(out, event_loop);
            }
        }
        
        LookupStatus::Searching
    }
    
    pub fn recv_finished(&mut self, out: &SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>)
        -> LookupStatus {
        // TODO: Announce to the appropriate nodes
        
        // This may not be cleared since we didnt set a timeout for each node,
        // any nodes that didnt respond would still be in here.
        self.active_lookups.clear();
        
        self.current_lookup_status()
    }
    
    fn current_lookup_status(&self) -> LookupStatus {
        if self.active_lookups.is_empty() {
            LookupStatus::Completed
        } else {
            LookupStatus::Searching
        }
    }
    
    fn is_closer_node(&self, prev_dist: &Distance, node: &Node) -> bool {
        self.target_id ^ node.id() < prev_dist
    }
    
    fn start_request_round(&mut self, conc_reqs: usize, out: &SyncSender<()>, event_loop: &mut EventLoop<DhtHandler>)
        -> bool {
        // Loop through nodes, number of concurrent requests desired
        for node_info in self.closest_nodes.iter_mut().filter(|&&mut (_, _, req)| !req).take(conc_reqs) {
            let (ref node_dist, ref node, ref mut req) = node_info;
            
            // Generate a transaction id for this message
            let trans_id = self.id_generator.generate();
            
            // Try to start a timeout for the node
            let res_timeout = event_loop.timeout_ms((0, ScheduledTask::CheckLookupTimeout(trans_id)), LOOKUP_TIMEOUT_MS);
            let timeout = if let Some(t) = res_timeout {
                t
            } else {
                error!("bip_dht: Failed to set a timeout for a table lookup...");
                return false
            }
            
            // Associate the transaction id with this node's distance and its timeout token
            self.active_lookups(trans_id, (*node_dist, timeout));
            
            // Send the message to the node
            let get_peers_msg = GetPeersRequest::new(trans_id.as_ref(), self.table_id, self.target_id).encode();
            if out.send((get_peers_msg, node.addr())).is_err() {
                error!("bip_dht: Could not send a lookup message through the channel...");
            }
            
            // Mark that we requested from the node
            *req = true;
        }
        
        true
    }
    
    fn start_endgame_round(&mut self, out: &SyncSender<(Vec<u8>, SocketAddr)>, event_loop: &mut EventLoop<DhtHandler>)
        -> bool {
        // Entering the endgame phase
        self.in_endgame = true;
        
        // Try to start a global message timeout for the endgame
        let res_timeout = event_loop.timeout_ms((0, ScheduledTask::CheckLookupTimeout(trans_id)), ENDGAME_TIMEOUT_MS);
        let timeout = if let Some(t) = res_timeout {
            t
        } else {
            error!("bip_dht: failed to set a timeout for table lookup endgame...");
            return false
        }
        
        // Request all unpinged nodes
        for node_info in self.closest_nodes.iter_mut().filter(|&&mut (_, _, req)| !req) {
            let (ref node_dist, ref node, ref mut req) = node_info;
            
            // Generate a transaction id for this message
            let trans_id = self.id_generator.generate();
            
            // Associate the transaction id with this node's distance and its timeout token
            // We dont actually need to keep track of this information, but we do still need to
            // filter out unsolicited responses by using the active_lookups map!!!
            self.active_lookups(trans_id, (*node_dist, timeout));
            
            // Send the message to the node
            let get_peers_msg = GetPeersRequest::new(trans_id.as_ref(), self.table_id, self.target_id).encode();
            if out.send((get_peers_msg, node.addr())).is_err() {
                error!("bip_dht: Could not send an endgame message through the channel...");
            }
            
            // Mark that we requested from the node
            *req = true;
        }
        
        true
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
                nodes.insert(dup_index, (node_dist, node, false));
            }
        },
        Err(ins_index) => nodes.insert(ins_index, (node_dist, node, false))
    };
}