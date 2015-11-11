use std::default::{Default};
use std::convert::{From};
use std::net::{SocketAddr, Ipv4Addr, SocketAddrV4};

use bip_util::{NodeId};
use bip_util::hash::{self, ShaHash};
use time::{Duration, PreciseTime};

// TODO: Should replace Node::new with Node::new_bad and Node::new_good (default is to create a bad node)
//   TODO: Remove the default impl for Node since new_bad and new_good would cause it to be ambiguous

/// Maximum wait period before a node becomes questionable.
const MAX_LAST_SEEN_MINS: i64 = 15;

/// Maximum number of requests before a Questionable node becomes Bad.
const MAX_REFRESH_REQUESTS: u8 = 2;

/// Status of the node.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum NodeStatus {
    Good,
    Questionable,
    Bad
}

/// Node participating in the DHT.
#[derive(Copy, Clone)]
pub struct Node {
    id:               NodeId,
    addr:             SocketAddr,
    last_request:     Option<PreciseTime>,
    last_response:    Option<PreciseTime>,
    refresh_requests: u8
}

impl Node {
    /// Creates a new node. All nodes implicitly start out with a Bad status.
    pub fn new(id: NodeId, addr: SocketAddr) -> Node {
        Node{ id: id, addr: addr, last_response: None, last_request: None, refresh_requests: 0 }
    }
    
    /// We sent the node a request.
    ///
    /// Panics if number of requests without a response would overflow an 8-bit number.
    pub fn local_request(&mut self) {
        match self.status() {
            NodeStatus::Good => (),
            _                => self.refresh_requests += 1
        };
    }
    
    /// Node sent us a request.
    pub fn remote_request(&mut self) {
        self.last_request = Some(PreciseTime::now());
    }
    
    /// Node sent us a response.
    pub fn remote_response(&mut self) {
        self.last_response = Some(PreciseTime::now());
        
        self.refresh_requests = 0;
    }
    
    /// Get the NodeId for the Node.
    pub fn id(&self) -> NodeId {
        self.id
    }
    
    /// Current status of the node.
    pub fn status(&self) -> NodeStatus {
        let curr_time = PreciseTime::now();
        
        match recently_responded(self, curr_time) {
            NodeStatus::Good         => return NodeStatus::Good,
            NodeStatus::Bad          => return NodeStatus::Bad,
            NodeStatus::Questionable => ()
        };
        
        recently_requested(self, curr_time)
    }
}

/// First scenario where a node is good is if it has responded to one of our requests recently.
///
/// Returns the status of the node where a Questionable status means the node has responded
/// to us before, but not recently.
fn recently_responded(node: &Node, curr_time: PreciseTime) -> NodeStatus {
    // Check if node has ever responded to us
    let since_response = match node.last_response {
        Some(response_time) => response_time.to(curr_time),
        None                => return NodeStatus::Bad
    };
    
    // Check if node has recently responded to us
    let max_last_response = Duration::minutes(MAX_LAST_SEEN_MINS);
    if since_response < max_last_response {
        NodeStatus::Good
    } else {
        NodeStatus::Questionable
    }
}

/// Second scenario where a node has ever responded to one of our requests and is good if it
/// has sent us a request recently.
///
/// Returns the final status of the node given that the first scenario found the node to be
/// Questionable.
fn recently_requested(node: &Node, curr_time: PreciseTime) -> NodeStatus {
    let max_last_request = Duration::minutes(MAX_LAST_SEEN_MINS);

    // Check if the node has recently request from us
    if let Some(request_time) = node.last_request {
        let since_request = request_time.to(curr_time);
        
        if since_request < max_last_request {
            return NodeStatus::Good
        }
    }
    
    // Check if we have request from node multiple times already without response
    if node.refresh_requests < MAX_REFRESH_REQUESTS {
        NodeStatus::Questionable
    } else {
        NodeStatus::Bad
    }
}

impl Default for Node {
    fn default() -> Node {
        let hash = ShaHash::from([0u8; hash::SHA_HASH_LEN]);
        
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let addr = SocketAddrV4::new(ip, 0);
        
        Node::new(hash, SocketAddr::V4(addr))
    }
}

#[cfg(test)]
mod tests {
    use std::convert::{From};
    use std::mem::{self};
    use std::net::{SocketAddr, Ipv4Addr, IpAddr, SocketAddrV4};
    
    use bip_util::{NodeId};
    use bip_util::hash::{self, ShaHash};
    use bip_util::test as bip_test;
    use time::{self, Duration, PreciseTime};
    
    use routing::node::{Node, NodeStatus};

    /// Returns a dummy socket address.
    fn dummy_socket_addr() -> SocketAddr {
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let addr = SocketAddrV4::new(ip, 0);
        
        SocketAddr::V4(addr)
    }
    
    /// Returns a dummy node id.
    fn dummy_node_id() -> NodeId {
        ShaHash::from([0u8; hash::SHA_HASH_LEN])
    }

    #[test]
    fn positive_initially_bad() {
        let node = Node::new(dummy_node_id(), dummy_socket_addr());
        
        assert_eq!(node.status(), NodeStatus::Bad);
    }
    
    #[test]
    fn positive_requested_bad() {
        let mut node = Node::new(dummy_node_id(), dummy_socket_addr());
        
        node.remote_request();
        
        assert_eq!(node.status(), NodeStatus::Bad);
    }
    
    #[test]
    fn positive_responded_good() {
        let mut node = Node::new(dummy_node_id(), dummy_socket_addr());
        
        node.remote_response();
        
        assert_eq!(node.status(), NodeStatus::Good);
    }
    
    #[test]
    fn posititve_responded_requested_good() {
        let mut node = Node::new(dummy_node_id(), dummy_socket_addr());
        
        node.remote_request();
        
        let time_offset = Duration::nanoseconds(1);
        let curr_time = bip_test::travel_into_future(time_offset);
        
        // Assumes node has responded to us recently
        let node_status = super::recently_requested(&node, curr_time);
        
        assert_eq!(node_status, NodeStatus::Good);
    }
    
    #[test]
    fn posititve_responded_requested_questionable() {
        let node = Node::new(dummy_node_id(), dummy_socket_addr());
        
        let time_offset = Duration::minutes(super::MAX_LAST_SEEN_MINS);
        let curr_time = bip_test::travel_into_future(time_offset);
        
        // Assumes node has responded to us recently
        let node_status = super::recently_requested(&node, curr_time);
        
        assert_eq!(node_status, NodeStatus::Questionable);
    }
    
    #[test]
    fn posititve_responded_requested_bad() {
        let mut node = Node::new(dummy_node_id(), dummy_socket_addr());
        
        for _ in 0..super::MAX_REFRESH_REQUESTS {
            node.local_request();
        }
        
        let time_offset = Duration::minutes(super::MAX_LAST_SEEN_MINS);
        let curr_time = bip_test::travel_into_future(time_offset);
        
        // Assumes node has responded to us recently
        let node_status = super::recently_requested(&node, curr_time);
        
        assert_eq!(node_status, NodeStatus::Bad);
    }
}