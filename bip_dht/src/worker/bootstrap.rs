use std::collections::{HashSet};
use std::net::{SocketAddr};
use std::sync::mpsc::{SyncSender};

use bip_util::{self, NodeId};

use message::find_node::{FindNodeRequest};
use routing::table::{self};

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
	active_bootstraps:      Vec<BucketBootstrap>,
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
		TableBootstrap{ table_node_id: id, active_bootstraps: Vec::new(), discovered_nodes: Vec::new(),
			discovered_routers: routers, next_bucket_hash_index: 0, next_bucket_node_index: 0 }
	}
	
	/// Returns true if the bootstrapping process has finished and false otherwise.
	pub fn check_bootstrap(&mut self, out: &SyncSender<(Vec<u8>, SocketAddr)>) -> bool {
		// Clear active bootstraps that are finished
		self.active_bootstraps.retain( |bootstrap|
			!bootstrap.is_done()
		);
		
		// Check if we can start more bootstraps
		let mut available_bootstraps = true;
		let mut bootstrap_finished = false;
		while available_bootstraps {
			match self.bootstrap_status() {
				BootstrapStatus::NextBootstrap(id, index) => {
					self.active_bootstraps.push(BucketBootstrap::new(id, index));
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
		for bucket_bootstrap in self.active_bootstraps.iter_mut() {
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
		} else if self.active_bootstraps.len() >= max_concurrent_bootstraps {
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
}