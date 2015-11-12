
const BOOTSTRAP_PROGRESS_BUCKET_SKIPS:         usize = 1;  // Buckets (bits) to skip per bootstrapped bucket
const BOOTSTRAP_PROGRESS_PINGS_PER_BUCKET:     usize = 16; // Nodes to ping per bootstrapped bucket
const BOOTSTRAP_PROGRESS_REQUEST_TIMEOUT_SECS: i64   = 2;  // Seconds to wait before declaring a request lost
const BOOTSTRAP_PROGRESS_NODES_PER_PROGRESS:   usize = 10; // Ratio of nodes to parallel progresses run
const BOOTSTRAP_PROGRESS_PARALLEL_REQUESTS:    usize = 5;  // Number of parallel requests per progress

struct BootstrapProgress {
	target_id:    NodeId,
	start_index:  usize,
	pinged_nodes: usize,
	last_updated: [PreciseTime; BOOTSTRAP_PROGRESS_PARALLEL_REQUESTS]
}

impl BootstrapProgress {
	pub fn new(target: NodeId, start: usize) -> BootstrapProgress {
		BootstrapProgress{ target_id: target, start_index: start, pinged_nodes: 0,
			last_updated: [PreciseTime::now(); BOOTSTRAP_PROGRESS_PARALLEL_REQUESTS] }
	}
	
	pub fn ping(&mut self, nodes: &[Node], send: &SyncSender<(Vec<u8>, SocketAddr)>) {
		
	}
	
	pub fn is_done(&self) -> bool {
		self.pinged_nodes >= BOOTSTRAP_PROGRESS_PINGS_PER_BUCKET
	}
	
	fn expired_time_index(&self) -> Option<usize> {
		let request_timeout = Duration::secs(BOOTSTRAP_PROGRESS_REQUEST_TIMEOUT_SECS);
		
		
	}
}

// To make bootstraps scalable, the number of bootstrap progresses that can be run in parallel
// should be a fraction of the number of discovered nodes for the current bootstrap process.
// This should prevent us from sending out too many requests to the same set of nodes in a small
// amount of time.

struct BootstrapProcess {
	table_node_id:      NodeId,
	active_bootstraps:  HashMap<TransactionId, BootstrapProgress>,
	discovered_nodes:   Vec<Node>,
	discovered_routers: HashSet<SocketAddr>,
	next_bucket_index:  usize
}

//----------------------------------------------------------------------------//

enum WorkerTask {
	
}