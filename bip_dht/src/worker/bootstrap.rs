use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::mpsc::SyncSender;

use bip_handshake::Handshaker;
use bip_util::bt::{self, NodeId};
use mio::{EventLoop, Timeout};

use crate::message::find_node::FindNodeRequest;
use crate::routing::bucket::Bucket;
use crate::routing::node::{Node, NodeStatus};
use crate::routing::table::{self, BucketContents, RoutingTable};
use crate::transaction::{MIDGenerator, TransactionID};
use crate::worker::handler::DhtHandler;
use crate::worker::ScheduledTask;

const BOOTSTRAP_INITIAL_TIMEOUT: u64 = 2500;
const BOOTSTRAP_NODE_TIMEOUT: u64 = 500;

const BOOTSTRAP_PINGS_PER_BUCKET: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub enum BootstrapStatus {
    /// Bootstrap has been finished.
    Idle,
    /// Bootstrap is in progress.
    Bootstrapping,
    /// Bootstrap just finished.
    Completed,
    /// Bootstrap failed in a fatal way.
    Failed,
}

pub struct TableBootstrap {
    table_id: NodeId,
    id_generator: MIDGenerator,
    starting_nodes: Vec<SocketAddr>,
    active_messages: HashMap<TransactionID, Timeout>,
    starting_routers: HashSet<SocketAddr>,
    curr_bootstrap_bucket: usize,
}

impl TableBootstrap {
    pub fn new<I>(
        table_id: NodeId,
        id_generator: MIDGenerator,
        nodes: Vec<SocketAddr>,
        routers: I,
    ) -> TableBootstrap
    where
        I: Iterator<Item = SocketAddr>,
    {
        let router_filter: HashSet<SocketAddr> = routers.collect();

        TableBootstrap {
            table_id,
            id_generator,
            starting_nodes: nodes,
            starting_routers: router_filter,
            active_messages: HashMap::new(),
            curr_bootstrap_bucket: 0,
        }
    }

    pub fn start_bootstrap<H>(
        &mut self,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> BootstrapStatus
    where
        H: Handshaker,
    {
        // Reset the bootstrap state
        self.active_messages.clear();
        self.curr_bootstrap_bucket = 0;

        // Generate transaction id for the initial bootstrap messages
        let trans_id = self.id_generator.generate();

        // Set a timer to begin the actual bootstrap
        let res_timeout = event_loop.timeout_ms(
            (
                BOOTSTRAP_INITIAL_TIMEOUT,
                ScheduledTask::CheckBootstrapTimeout(trans_id),
            ),
            BOOTSTRAP_INITIAL_TIMEOUT,
        );
        let timeout = if let Ok(t) = res_timeout {
            t
        } else {
            error!("bip_dht: Failed to set a timeout for the start of a table bootstrap...");
            return BootstrapStatus::Failed;
        };

        // Insert the timeout into the active bootstraps just so we can check if a
        // response was valid (and begin the bucket bootstraps)
        self.active_messages.insert(trans_id, timeout);

        let find_node_msg =
            FindNodeRequest::new(trans_id.as_ref(), self.table_id, self.table_id).encode();
        // Ping all initial routers and nodes
        for addr in self
            .starting_routers
            .iter()
            .chain(self.starting_nodes.iter())
        {
            if out.send((find_node_msg.clone(), *addr)).is_err() {
                error!("bip_dht: Failed to send bootstrap message to router through channel...");
                return BootstrapStatus::Failed;
            }
        }

        self.current_bootstrap_status()
    }

    pub fn is_router(&self, addr: &SocketAddr) -> bool {
        self.starting_routers.contains(&addr)
    }

    pub fn recv_response<'a, H>(
        &mut self,
        trans_id: &TransactionID,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> BootstrapStatus
    where
        H: Handshaker,
    {
        // Process the message transaction id
        let timeout = if let Some(t) = self.active_messages.get(trans_id) {
            *t
        } else {
            warn!(
                "bip_dht: Received expired/unsolicited node response for an active table \
                   bootstrap..."
            );
            return self.current_bootstrap_status();
        };

        // If this response was from the initial bootstrap, we don't want to clear the
        // timeout or remove the token from the map as we want to wait until the
        // proper timeout has been triggered before starting
        if self.curr_bootstrap_bucket != 0 {
            // Message was not from the initial ping
            // Remove the timeout from the event loop
            event_loop.clear_timeout(timeout);

            // Remove the token from the mapping
            self.active_messages.remove(trans_id);
        }

        // Check if we need to bootstrap on the next bucket
        if self.active_messages.is_empty() {
            return self.bootstrap_next_bucket(table, out, event_loop);
        }

        self.current_bootstrap_status()
    }

    pub fn recv_timeout<H>(
        &mut self,
        trans_id: &TransactionID,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> BootstrapStatus
    where
        H: Handshaker,
    {
        if self.active_messages.remove(trans_id).is_none() {
            warn!(
                "bip_dht: Received expired/unsolicited node timeout for an active table \
                   bootstrap..."
            );
            return self.current_bootstrap_status();
        }

        // Check if we need to bootstrap on the next bucket
        if self.active_messages.is_empty() {
            return self.bootstrap_next_bucket(table, out, event_loop);
        }

        self.current_bootstrap_status()
    }

    // Returns true if there are more buckets to bootstrap, false otherwise
    fn bootstrap_next_bucket<H>(
        &mut self,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> BootstrapStatus
    where
        H: Handshaker,
    {
        let target_id = flip_id_bit_at_index(self.table_id, self.curr_bootstrap_bucket);

        // Get the optimal iterator to bootstrap the current bucket
        if self.curr_bootstrap_bucket == 0 || self.curr_bootstrap_bucket == 1 {
            let iter = table
                .closest_nodes(target_id)
                .filter(|n| n.status() == NodeStatus::Questionable);

            self.send_bootstrap_requests(iter, target_id, table, out, event_loop)
        } else {
            let mut buckets = table.buckets().skip(self.curr_bootstrap_bucket - 2);
            let dummy_bucket = Bucket::new();

            // Sloppy probabilities of our target node residing at the node
            let percent_25_bucket = if let Some(bucket) = buckets.next() {
                match bucket {
                    BucketContents::Empty => dummy_bucket.iter(),
                    BucketContents::Sorted(b) => b.iter(),
                    BucketContents::Assorted(b) => b.iter(),
                }
            } else {
                dummy_bucket.iter()
            };
            let percent_50_bucket = if let Some(bucket) = buckets.next() {
                match bucket {
                    BucketContents::Empty => dummy_bucket.iter(),
                    BucketContents::Sorted(b) => b.iter(),
                    BucketContents::Assorted(b) => b.iter(),
                }
            } else {
                dummy_bucket.iter()
            };
            let percent_100_bucket = if let Some(bucket) = buckets.next() {
                match bucket {
                    BucketContents::Empty => dummy_bucket.iter(),
                    BucketContents::Sorted(b) => b.iter(),
                    BucketContents::Assorted(b) => b.iter(),
                }
            } else {
                dummy_bucket.iter()
            };

            // TODO: Figure out why chaining them in reverse gives us more total nodes on
            // average, perhaps it allows us to fill up the lower buckets faster
            // at the cost of less nodes in the higher buckets (since lower buckets are very
            // easy to fill)...Although it should even out since we are
            // stagnating buckets, so doing it in reverse may make sense since on the 3rd
            // iteration, it allows us to ping questionable nodes in our first
            // buckets right off the bat.
            let iter = percent_25_bucket
                .chain(percent_50_bucket)
                .chain(percent_100_bucket)
                .filter(|n| n.status() == NodeStatus::Questionable);

            self.send_bootstrap_requests(iter, target_id, table, out, event_loop)
        }
    }

    fn send_bootstrap_requests<'a, H, I>(
        &mut self,
        nodes: I,
        target_id: NodeId,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> BootstrapStatus
    where
        I: Iterator<Item = &'a Node>,
        H: Handshaker,
    {
        info!(
            "bip_dht: bootstrap::send_bootstrap_requests {}",
            self.curr_bootstrap_bucket
        );

        let mut messages_sent = 0;

        for node in nodes.take(BOOTSTRAP_PINGS_PER_BUCKET) {
            // Generate a transaction id
            let trans_id = self.id_generator.generate();
            let find_node_msg =
                FindNodeRequest::new(trans_id.as_ref(), self.table_id, target_id).encode();

            // Add a timeout for the node
            let res_timeout = event_loop.timeout_ms(
                (
                    BOOTSTRAP_NODE_TIMEOUT,
                    ScheduledTask::CheckBootstrapTimeout(trans_id),
                ),
                BOOTSTRAP_NODE_TIMEOUT,
            );
            let timeout = if let Ok(t) = res_timeout {
                t
            } else {
                error!("bip_dht: Failed to set a timeout for the start of a table bootstrap...");
                return BootstrapStatus::Failed;
            };

            // Send the message to the node
            if out.send((find_node_msg, node.addr())).is_err() {
                error!("bip_dht: Could not send a bootstrap message through the channel...");
                return BootstrapStatus::Failed;
            }

            // Mark that we requested from the node
            node.local_request();

            // Create an entry for the timeout in the map
            self.active_messages.insert(trans_id, timeout);

            messages_sent += 1;
        }

        self.curr_bootstrap_bucket += 1;
        if self.curr_bootstrap_bucket == table::MAX_BUCKETS {
            BootstrapStatus::Completed
        } else if messages_sent == 0 {
            self.bootstrap_next_bucket(table, out, event_loop)
        } else {
            BootstrapStatus::Bootstrapping
        }
    }

    fn current_bootstrap_status(&self) -> BootstrapStatus {
        if self.curr_bootstrap_bucket == table::MAX_BUCKETS || self.active_messages.is_empty() {
            BootstrapStatus::Idle
        } else {
            BootstrapStatus::Bootstrapping
        }
    }
}

/// Panics if index is out of bounds.
/// TODO: Move into bip_util crate
fn flip_id_bit_at_index(node_id: NodeId, index: usize) -> NodeId {
    let mut id_bytes: [u8; bt::NODE_ID_LEN] = node_id.into();
    let (byte_index, bit_index) = (index / 8, index % 8);

    let actual_bit_index = 7 - bit_index;
    id_bytes[byte_index] ^= 1 << actual_bit_index;

    id_bytes.into()
}
