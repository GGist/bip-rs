use std::default::{Default};
use std::net::{Ipv4Addr, SocketAddrV4, SocketAddr};
use std::slice::{Iter};

use bip_util::{self, NodeId};

use routing::node::{Node, NodeStatus};

/// Maximum number of nodes that should reside in any bucket.
pub const MAX_BUCKET_SIZE: usize = 8;

/// Bucket containing Nodes with identical bit prefixes.
pub struct Bucket {
    nodes: [Node; MAX_BUCKET_SIZE]
}

impl Bucket {
    /// Create a new Bucket with all Nodes default initialized.
    pub fn new() -> Bucket {
        let id = NodeId::from([0u8; bip_util::NODE_ID_LEN]);
        
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        
        Bucket{ nodes: [Node::as_bad(id, addr), Node::as_bad(id, addr), Node::as_bad(id, addr), Node::as_bad(id, addr),
            Node::as_bad(id, addr), Node::as_bad(id, addr), Node::as_bad(id, addr), Node::as_bad(id, addr)] }
    }
    
    /// Iterator over each node within the bucket.
    ///
    /// For buckets newly created, the initial bad nodes are included.
    pub fn iter(&self) -> Iter<Node> {
        self.nodes.iter()
    }
    
    /// Indicates if the bucket needs to be refreshed.
    pub fn needs_refresh(&self) -> bool {
        self.nodes.iter().fold(true, |prev, node| prev && node.status() != NodeStatus::Good )
    }
    
    /// Attempt to add the given Node to the bucket if it is not in a bad state.
    ///
    /// Returns false if the Node could not be placed in the bucket because it is full.
    pub fn add_node(&mut self, new_node: Node) -> bool {
        let new_node_status = new_node.status();
        if new_node_status == NodeStatus::Bad {
            return true
        }
        
        // See if this node is already in the table, in that case replace it
        if let Some(index) = self.nodes.iter().position(|node| *node == new_node) {
            let node_status = self.nodes[index].status();
            
            if new_node_status == NodeStatus::Good {
                self.nodes[index] = new_node;
            } else if node_status != NodeStatus::Good {
                self.nodes[index] = new_node;
            }
            
            return true
        }
        
        // See if any lower priority nodes are present in the table
        let replace_index = if new_node_status == NodeStatus::Good {
            self.nodes.iter().position(|node| {
                let status = node.status();
                
                status == NodeStatus::Questionable || status == NodeStatus::Bad
            })
        } else {
            self.nodes.iter().position(|node| {
                let status = node.status();
                
                status == NodeStatus::Bad
            })
        };
        
        if let Some(index) = replace_index {
            self.nodes[index] = new_node;
            
            true
        } else {
            false
        }
    }
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