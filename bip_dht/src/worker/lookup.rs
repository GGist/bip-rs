use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::mpsc::SyncSender;

use bip_handshake::Handshaker;
use bip_util::bt::{self, InfoHash, NodeId};
use bip_util::net;
use bip_util::sha::ShaHash;
use mio::{EventLoop, Timeout};

use crate::message::announce_peer::{AnnouncePeerRequest, ConnectPort};
use crate::message::get_peers::{CompactInfoType, GetPeersRequest, GetPeersResponse};
use crate::routing::bucket;
use crate::routing::node::{Node, NodeStatus};
use crate::routing::table::RoutingTable;
use crate::transaction::{MIDGenerator, TransactionID};
use crate::worker::handler::DhtHandler;
use crate::worker::ScheduledTask;

const LOOKUP_TIMEOUT_MS: u64 = 1500;
const ENDGAME_TIMEOUT_MS: u64 = 1500;

// Currently using the aggressive variant of the standard lookup procedure.
// https://people.kth.se/~rauljc/p2p11/jimenez2011subsecond.pdf

// TODO: Handle case where a request round fails, should we fail the whole
// lookup (clear acvite lookups?) TODO: Clean up the code in this module.

const INITIAL_PICK_NUM: usize = 4; // Alpha
const ITERATIVE_PICK_NUM: usize = 3; // Beta
const ANNOUNCE_PICK_NUM: usize = 8; // # Announces

type Distance = ShaHash;
type DistanceToBeat = ShaHash;

#[derive(Debug, PartialEq, Eq)]
pub enum LookupStatus {
    Searching,
    Values(Vec<SocketAddrV4>),
    Completed,
    Failed,
}

pub struct TableLookup {
    table_id: NodeId,
    target_id: InfoHash,
    in_endgame: bool,
    // If we have received any values in the lookup.
    recv_values: bool,
    id_generator: MIDGenerator,
    will_announce: bool,
    // DistanceToBeat is the distance that the responses of the current lookup needs to beat,
    // interestingly enough (and super important), this distance may not be eqaul to the
    // requested node's distance
    active_lookups: HashMap<TransactionID, (DistanceToBeat, Timeout)>,
    announce_tokens: HashMap<Node, Vec<u8>>,
    requested_nodes: HashSet<Node>,
    // Storing whether or not it has ever been pinged so that we
    // can perform the brute force lookup if the lookup failed
    all_sorted_nodes: Vec<(Distance, Node, bool)>,
}

// Gather nodes

impl TableLookup {
    pub fn new<H>(
        table_id: NodeId,
        target_id: InfoHash,
        id_generator: MIDGenerator,
        will_announce: bool,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> Option<TableLookup>
    where
        H: Handshaker,
    {
        // Pick a buckets worth of nodes and put them into the all_sorted_nodes list
        let mut all_sorted_nodes = Vec::with_capacity(bucket::MAX_BUCKET_SIZE);
        for node in table
            .closest_nodes(target_id)
            .filter(|n| n.status() == NodeStatus::Good)
            .take(bucket::MAX_BUCKET_SIZE)
        {
            insert_sorted_node(&mut all_sorted_nodes, target_id, node.clone(), false);
        }

        // Call pick_initial_nodes with the all_sorted_nodes list as an iterator
        let initial_pick_nodes = pick_initial_nodes(all_sorted_nodes.iter_mut());
        let initial_pick_nodes_filtered =
            initial_pick_nodes
                .iter()
                .filter(|&&(_, good)| good)
                .map(|&(ref node, _)| {
                    let distance_to_beat = node.id() ^ target_id;

                    (node, distance_to_beat)
                });

        // Construct the lookup table structure
        let mut table_lookup = TableLookup {
            table_id,
            target_id,
            in_endgame: false,
            recv_values: false,
            id_generator,
            will_announce,
            all_sorted_nodes,
            announce_tokens: HashMap::new(),
            requested_nodes: HashSet::new(),
            active_lookups: HashMap::with_capacity(INITIAL_PICK_NUM),
        };

        // Call start_request_round with the list of initial_nodes (return even if the
        // search completed...for now :D)
        if table_lookup.start_request_round(initial_pick_nodes_filtered, table, out, event_loop)
            != LookupStatus::Failed
        {
            Some(table_lookup)
        } else {
            None
        }
    }

    pub fn info_hash(&self) -> InfoHash {
        self.target_id
    }

    pub fn recv_response<'a, H>(
        &mut self,
        node: Node,
        trans_id: &TransactionID,
        msg: GetPeersResponse<'a>,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> LookupStatus
    where
        H: Handshaker,
    {
        // Process the message transaction id
        let (dist_to_beat, timeout) = if let Some(lookup) = self.active_lookups.remove(trans_id) {
            lookup
        } else {
            warn!(
                "bip_dht: Received expired/unsolicited node response for an active table \
                   lookup..."
            );
            return self.current_lookup_status();
        };

        // Cancel the timeout (if this is not an endgame response)
        if !self.in_endgame {
            event_loop.clear_timeout(timeout);
        }

        // Add the announce token to our list of tokens
        if let Some(token) = msg.token() {
            self.announce_tokens.insert(node, token.to_vec());
        }

        // Pull out the contact information from the message
        let (opt_values, opt_nodes) = match msg.info_type() {
            CompactInfoType::Nodes(n) => (None, Some(n)),
            CompactInfoType::Values(v) => {
                self.recv_values = true;
                (Some(v.into_iter().collect()), None)
            }
            CompactInfoType::Both(n, v) => (Some(v.into_iter().collect()), Some(n)),
        };

        // Check if we beat the distance, get the next distance to beat
        let (iterate_nodes, next_dist_to_beat) = if let Some(nodes) = opt_nodes {
            let requested_nodes = &self.requested_nodes;

            // Filter for nodes that we have already requested from
            let already_requested = |node_info: &(NodeId, SocketAddrV4)| {
                let node = Node::as_questionable(node_info.0, SocketAddr::V4(node_info.1));

                !requested_nodes.contains(&node)
            };

            // Get the closest distance (or the current distance)
            let next_dist_to_beat = nodes.into_iter().filter(&already_requested).fold(
                dist_to_beat,
                |closest, (id, _)| {
                    let distance = self.target_id ^ id;

                    if distance < closest {
                        distance
                    } else {
                        closest
                    }
                },
            );

            // Check if we got closer (equal to is not enough)
            let iterate_nodes = if next_dist_to_beat < dist_to_beat {
                let iterate_nodes = pick_iterate_nodes(
                    nodes.into_iter().filter(&already_requested),
                    self.target_id,
                );

                // Push nodes into the all nodes list
                for (id, v4_addr) in nodes {
                    let addr = SocketAddr::V4(v4_addr);
                    let node = Node::as_questionable(id, addr);
                    let will_ping = iterate_nodes.iter().any(|&(ref n, _)| n == &node);

                    insert_sorted_node(&mut self.all_sorted_nodes, self.target_id, node, will_ping);
                }

                Some(iterate_nodes)
            } else {
                // Push nodes into the all nodes list
                for (id, v4_addr) in nodes {
                    let addr = SocketAddr::V4(v4_addr);
                    let node = Node::as_questionable(id, addr);

                    insert_sorted_node(&mut self.all_sorted_nodes, self.target_id, node, false);
                }

                None
            };

            (iterate_nodes, next_dist_to_beat)
        } else {
            (None, dist_to_beat)
        };

        // Check if we need to iterate (not in the endgame already)
        if !self.in_endgame {
            // If the node gave us a closer id than its own to the target id, continue the
            // search
            if let Some(ref nodes) = iterate_nodes {
                let filtered_nodes = nodes
                    .iter()
                    .filter(|&&(_, good)| good)
                    .map(|&(ref n, _)| (n, next_dist_to_beat));
                if self.start_request_round(filtered_nodes, table, out, event_loop)
                    == LookupStatus::Failed
                {
                    return LookupStatus::Failed;
                }
            }

            // If there are not more active lookups, start the endgame
            if self.active_lookups.is_empty()
                && self.start_endgame_round(table, out, event_loop) == LookupStatus::Failed
            {
                return LookupStatus::Failed;
            }
        }

        match opt_values {
            Some(values) => LookupStatus::Values(values),
            None => self.current_lookup_status(),
        }
    }

    pub fn recv_timeout<H>(
        &mut self,
        trans_id: &TransactionID,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> LookupStatus
    where
        H: Handshaker,
    {
        if self.active_lookups.remove(trans_id).is_none() {
            warn!(
                "bip_dht: Received expired/unsolicited node timeout for an active table \
                   lookup..."
            );
            return self.current_lookup_status();
        }

        if !self.in_endgame {
            // If there are not more active lookups, start the endgame
            if self.active_lookups.is_empty()
                && self.start_endgame_round(table, out, event_loop) == LookupStatus::Failed
            {
                return LookupStatus::Failed;
            }
        }

        self.current_lookup_status()
    }

    pub fn recv_finished(
        &mut self,
        handshake_port: u16,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
    ) -> LookupStatus {
        let mut fatal_error = false;

        // Announce if we were told to
        if self.will_announce {
            // Partial borrow so the filter function doesnt capture all of self
            let announce_tokens = &self.announce_tokens;

            for &(_, ref node, _) in self
                .all_sorted_nodes
                .iter()
                .filter(|&&(_, ref node, _)| announce_tokens.contains_key(node))
                .take(ANNOUNCE_PICK_NUM)
            {
                let trans_id = self.id_generator.generate();
                let token = announce_tokens.get(node).unwrap();

                let announce_peer_req = AnnouncePeerRequest::new(
                    trans_id.as_ref(),
                    self.table_id,
                    self.target_id,
                    token.as_ref(),
                    ConnectPort::Explicit(handshake_port),
                );
                let announce_peer_msg = announce_peer_req.encode();

                if out.send((announce_peer_msg, node.addr())).is_err() {
                    error!(
                        "bip_dht: TableLookup announce request failed to send through the out \
                            channel..."
                    );
                    fatal_error = true;
                }

                if !fatal_error {
                    // We requested from the node, marke it down if the node is in our routing table
                    if let Some(n) = table.find_node(node) {
                        n.local_request()
                    }
                }
            }
        }

        // This may not be cleared since we didnt set a timeout for each node, any nodes
        // that didnt respond would still be in here.
        self.active_lookups.clear();
        self.in_endgame = false;

        if fatal_error {
            LookupStatus::Failed
        } else {
            self.current_lookup_status()
        }
    }

    fn current_lookup_status(&self) -> LookupStatus {
        if self.in_endgame || !self.active_lookups.is_empty() {
            LookupStatus::Searching
        } else {
            LookupStatus::Completed
        }
    }

    fn start_request_round<'a, H, I>(
        &mut self,
        nodes: I,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> LookupStatus
    where
        I: Iterator<Item = (&'a Node, DistanceToBeat)>,
        H: Handshaker,
    {
        // Loop through the given nodes
        let mut messages_sent = 0;
        for (node, dist_to_beat) in nodes {
            // Generate a transaction id for this message
            let trans_id = self.id_generator.generate();

            // Try to start a timeout for the node
            let res_timeout = event_loop.timeout_ms(
                (0, ScheduledTask::CheckLookupTimeout(trans_id)),
                LOOKUP_TIMEOUT_MS,
            );
            let timeout = if let Ok(t) = res_timeout {
                t
            } else {
                error!("bip_dht: Failed to set a timeout for a table lookup...");
                return LookupStatus::Failed;
            };

            // Associate the transaction id with the distance the returned nodes must beat
            // and the timeout token
            self.active_lookups
                .insert(trans_id, (dist_to_beat, timeout));

            // Send the message to the node
            let get_peers_msg =
                GetPeersRequest::new(trans_id.as_ref(), self.table_id, self.target_id).encode();
            if out.send((get_peers_msg, node.addr())).is_err() {
                error!("bip_dht: Could not send a lookup message through the channel...");
                return LookupStatus::Failed;
            }

            // We requested from the node, mark it down
            self.requested_nodes.insert(node.clone());

            // Update the node in the routing table
            if let Some(n) = table.find_node(node) {
                n.local_request()
            }

            messages_sent += 1;
        }

        if messages_sent == 0 {
            self.active_lookups.clear();
            LookupStatus::Completed
        } else {
            LookupStatus::Searching
        }
    }

    fn start_endgame_round<H>(
        &mut self,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> LookupStatus
    where
        H: Handshaker,
    {
        // Entering the endgame phase
        self.in_endgame = true;

        // Try to start a global message timeout for the endgame
        let res_timeout = event_loop.timeout_ms(
            (
                0,
                ScheduledTask::CheckLookupEndGame(self.id_generator.generate()),
            ),
            ENDGAME_TIMEOUT_MS,
        );
        let timeout = if let Ok(t) = res_timeout {
            t
        } else {
            error!("bip_dht: Failed to set a timeout for table lookup endgame...");
            return LookupStatus::Failed;
        };

        // Request all unpinged nodes if we didnt receive any values
        if !self.recv_values {
            for node_info in self
                .all_sorted_nodes
                .iter_mut()
                .filter(|&&mut (_, _, req)| !req)
            {
                let &mut (ref node_dist, ref node, ref mut req) = node_info;

                // Generate a transaction id for this message
                let trans_id = self.id_generator.generate();

                // Associate the transaction id with this node's distance and its timeout token
                // We dont actually need to keep track of this information, but we do still need
                // to filter out unsolicited responses by using the
                // active_lookups map!!!
                self.active_lookups.insert(trans_id, (*node_dist, timeout));

                // Send the message to the node
                let get_peers_msg =
                    GetPeersRequest::new(trans_id.as_ref(), self.table_id, self.target_id).encode();
                if out.send((get_peers_msg, node.addr())).is_err() {
                    error!("bip_dht: Could not send an endgame message through the channel...");
                    return LookupStatus::Failed;
                }

                // Mark that we requested from the node in the RoutingTable
                if let Some(n) = table.find_node(node) {
                    n.local_request()
                }

                // Mark that we requested from the node
                *req = true;
            }
        }

        LookupStatus::Searching
    }
}

/// Picks a number of nodes from the sorted distance iterator to ping on the
/// first round.
fn pick_initial_nodes<'a, I>(sorted_nodes: I) -> [(Node, bool); INITIAL_PICK_NUM]
where
    I: Iterator<Item = &'a mut (Distance, Node, bool)>,
{
    let dummy_id = [0u8; bt::NODE_ID_LEN].into();
    let default = (Node::as_bad(dummy_id, net::default_route_v4()), false);

    let mut pick_nodes = [default.clone(), default.clone(), default.clone(), default];
    for (src, dst) in sorted_nodes.zip(pick_nodes.iter_mut()) {
        dst.0 = src.1.clone();
        dst.1 = true;

        // Mark that the node has been requested from
        src.2 = true;
    }

    pick_nodes
}

/// Picks a number of nodes from the unsorted distance iterator to ping on
/// iterative rounds.
fn pick_iterate_nodes<I>(
    unsorted_nodes: I,
    target_id: InfoHash,
) -> [(Node, bool); ITERATIVE_PICK_NUM]
where
    I: Iterator<Item = (NodeId, SocketAddrV4)>,
{
    let dummy_id = [0u8; bt::NODE_ID_LEN].into();
    let default = (Node::as_bad(dummy_id, net::default_route_v4()), false);

    let mut pick_nodes = [default.clone(), default.clone(), default];
    for (id, v4_addr) in unsorted_nodes {
        let addr = SocketAddr::V4(v4_addr);
        let node = Node::as_questionable(id, addr);

        insert_closest_nodes(&mut pick_nodes, target_id, node);
    }

    pick_nodes
}

/// Inserts the node into the slice if a slot in the slice is unused or a node
/// in the slice is further from the target id than the node being inserted.
fn insert_closest_nodes(nodes: &mut [(Node, bool)], target_id: InfoHash, new_node: Node) {
    let new_distance = target_id ^ new_node.id();

    for &mut (ref mut old_node, ref mut used) in nodes.iter_mut() {
        if !*used {
            // Slot was not in use, go ahead and place the node
            *old_node = new_node;
            *used = true;
            return;
        } else {
            // Slot is in use, see if our node is closer to the target
            let old_distance = target_id ^ old_node.id();

            if new_distance < old_distance {
                *old_node = new_node;
                return;
            }
        }
    }
}

/// Inserts the Node into the list of nodes based on its distance from the
/// target node.
///
/// Nodes at the start of the list are closer to the target node than nodes at
/// the end.
fn insert_sorted_node(
    nodes: &mut Vec<(Distance, Node, bool)>,
    target: InfoHash,
    node: Node,
    pinged: bool,
) {
    let node_id = node.id();
    let node_dist = target ^ node_id;

    // Perform a search by distance from the target id
    let search_result = nodes.binary_search_by(|&(dist, _, _)| dist.cmp(&node_dist));
    match search_result {
        Ok(dup_index) => {
            // TODO: Bug here, what happens when multiple nodes with the same distance are
            // present, but we dont get the index of the duplicate node (its in the list)
            // from the search, then we would have a duplicate node in the list!
            // Insert only if this node is different (it is ok if they have the same id)
            if nodes[dup_index].1 != node {
                nodes.insert(dup_index, (node_dist, node, pinged));
            }
        }
        Err(ins_index) => nodes.insert(ins_index, (node_dist, node, pinged)),
    };
}
