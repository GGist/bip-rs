use std::net::SocketAddr;
use std::sync::mpsc::SyncSender;

use bip_handshake::Handshaker;
use bip_util::bt::{self, NodeId};
use mio::EventLoop;

use crate::message::find_node::FindNodeRequest;
use crate::routing::node::NodeStatus;
use crate::routing::table::{self, RoutingTable};
use crate::transaction::MIDGenerator;
use crate::worker::handler::DhtHandler;
use crate::worker::ScheduledTask;

const REFRESH_INTERVAL_TIMEOUT: u64 = 6000;

pub enum RefreshStatus {
    /// Refresh is in progress.
    Refreshing,
    /// Refresh failed in a fatal way.
    Failed,
}

pub struct TableRefresh {
    id_generator: MIDGenerator,
    curr_refresh_bucket: usize,
}

impl TableRefresh {
    pub fn new(id_generator: MIDGenerator) -> TableRefresh {
        TableRefresh {
            id_generator,
            curr_refresh_bucket: 0,
        }
    }

    pub fn continue_refresh<H>(
        &mut self,
        table: &RoutingTable,
        out: &SyncSender<(Vec<u8>, SocketAddr)>,
        event_loop: &mut EventLoop<DhtHandler<H>>,
    ) -> RefreshStatus
    where
        H: Handshaker,
    {
        if self.curr_refresh_bucket == table::MAX_BUCKETS {
            self.curr_refresh_bucket = 0;
        }
        let target_id = flip_id_bit_at_index(table.node_id(), self.curr_refresh_bucket);

        info!(
            "bip_dht: Performing a refresh for bucket {}",
            self.curr_refresh_bucket
        );
        // Ping the closest questionable node
        for node in table
            .closest_nodes(target_id)
            .filter(|n| n.status() == NodeStatus::Questionable)
            .take(1)
        {
            // Generate a transaction id for the request
            let trans_id = self.id_generator.generate();

            // Construct the message
            let find_node_req = FindNodeRequest::new(trans_id.as_ref(), table.node_id(), target_id);
            let find_node_msg = find_node_req.encode();

            // Send the message
            if out.send((find_node_msg, node.addr())).is_err() {
                error!(
                    "bip_dht: TableRefresh failed to send a refresh message to the out \
                        channel..."
                );
                return RefreshStatus::Failed;
            }

            // Mark that we requested from the node
            node.local_request();
        }

        // Generate a dummy transaction id (only the action id will be used)
        let trans_id = self.id_generator.generate();

        // Start a timer for the next refresh
        if event_loop
            .timeout_ms(
                (0, ScheduledTask::CheckTableRefresh(trans_id)),
                REFRESH_INTERVAL_TIMEOUT,
            )
            .is_err()
        {
            error!("bip_dht: TableRefresh failed to set a timeout for the next refresh...");
            return RefreshStatus::Failed;
        }

        self.curr_refresh_bucket += 1;

        RefreshStatus::Refreshing
    }
}

/// Panics if index is out of bounds.
fn flip_id_bit_at_index(node_id: NodeId, index: usize) -> NodeId {
    let mut id_bytes: [u8; bt::NODE_ID_LEN] = node_id.into();
    let (byte_index, bit_index) = (index / 8, index % 8);

    let actual_bit_index = 7 - bit_index;
    id_bytes[byte_index] ^= 1 << actual_bit_index;

    id_bytes.into()
}
