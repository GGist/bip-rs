use std::default::{Default};
use std::slice::{Iter};

use time::{PreciseTime, Duration};

use routing::node::{Node, NodeStatus};

/// Maximum wait period before a bucket should be refreshed.
const MAX_LAST_REFRESH_MINS: i64 = 15;

/// Maximum number of nodes that should reside in any bucket.
pub const MAX_BUCKET_SIZE: usize = 8;

/// Bucket containing Nodes with identical bit prefixes.
#[derive(Copy, Clone)]
pub struct Bucket {
    nodes:        [Node; MAX_BUCKET_SIZE],
    last_changed: PreciseTime
}

impl Bucket {
    /// Create a new Bucket with all Nodes default initialized.
    pub fn new() -> Bucket {
        Bucket{ nodes: [Default::default(); MAX_BUCKET_SIZE], last_changed: PreciseTime::now() }
    }
    
    /// Iterator over each node within the bucket.
    ///
    /// For buckets newly created, the initial bad nodes are included.
    pub fn iter(&self) -> Iter<Node> {
        self.nodes.iter()
    }
    
    /// Indicates if the bucket needs to be refreshed.
    pub fn needs_refresh(&self) -> bool {
        let max_refresh = Duration::minutes(MAX_LAST_REFRESH_MINS);
        
        self.last_changed.to(PreciseTime::now()) > max_refresh
    }
    
    /// Manually trigger a bucket refresh, this resets the timer that indicates when
    /// the bucket needs to be refreshed. This should only be called if a Node in
    /// the bucket was requested from and responded.
    pub fn trigger_refresh(&mut self) {
        self.last_changed = PreciseTime::now();
    }
    
    /// Add the given node to the Bucket if it is in a Good status.
    ///
    /// Returns false if the Bucket is full otherwise returns true. Note
    /// that just because it returns true does not mean it was actually added.
    pub fn add_node(&mut self, node: Node) -> bool {
        if node.status() != NodeStatus::Good {
            return true
        }
    
        match first_bad_node(&self.nodes[..]) {
            Some(pos) => {
                self.nodes[pos] = node;
                self.trigger_refresh();
                true
            },
            None => false
        }
    }
}

/// Returns the position of the first Bad node.
fn first_bad_node(nodes: &[Node]) -> Option<usize> {
    nodes.iter().position(|node| node.status() == NodeStatus::Bad)
}

#[cfg(test)]
mod tests {
    use std::default::{Default};
    use std::net::{ToSocketAddrs};

    use bip_util::hash::{self, ShaHash};

    use routing::bucket::{self, Bucket};
    use routing::node::{Node};

    #[test]
    fn positive_full_bucket() {
        let mut bucket = Bucket::new();
    
        for _ in 0..bucket::MAX_BUCKET_SIZE {
            let mut node = Node::default();
            // Make sure the node is good
            node.remote_response();
            
            assert!(bucket.add_node(node));
        }
        
        let mut node = Node::default();
        node.remote_response();
        assert!(!bucket.add_node(node));
    }
    
    #[test]
    fn positive_bad_node_ignored() {
        let mut bucket = Bucket::new();
        
        let mut node_bytes = [0u8; hash::SHA_HASH_LEN];
        for byte in node_bytes.iter_mut() {
            // Random number, default bucket nodes are initialized to 0
            *byte = 213;
        }
        let node_id = ShaHash::from_bytes(&node_bytes[..]).unwrap();
        let node = Node::new(node_id, "127.0.0.1:0".to_socket_addrs().unwrap().next().unwrap());
        
        // Shouldnt have actually added our bad node, but bucket shouldnt be full either
        assert!(bucket.add_node(node));
        
        assert!(!bucket.iter().any(|&n| n.id() == node.id()));
    }
}