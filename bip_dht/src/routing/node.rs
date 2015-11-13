use std::cell::{Cell};
use std::default::{Default};
use std::convert::{From};
use std::net::{SocketAddr, Ipv4Addr, SocketAddrV4};
use std::sync::atomic::{AtomicUsize, Ordering};

use bip_util::{NodeId};
use bip_util::hash::{self, ShaHash};
use bip_util::test::{self};
use chrono::{Duration, DateTime, UTC};

// TODO: Should replace Node::new with Node::new_bad and Node::new_good (default is to create a bad node)
//   TODO: Remove the default impl for Node since new_bad and new_good would cause it to be ambiguous

/// Maximum wait period before a node becomes questionable.
const MAX_LAST_SEEN_MINS: i64 = 15;

/// Maximum number of requests before a Questionable node becomes Bad.
const MAX_REFRESH_REQUESTS: usize = 2;

/// Status of the node.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum NodeStatus {
    Good,
    Questionable,
    Bad
}

/// Node participating in the dht.
pub struct Node {
    id:               NodeId,
    addr:             SocketAddr,
    last_request:     Cell<Option<DateTime<UTC>>>,
    last_response:    Cell<Option<DateTime<UTC>>>,
    refresh_requests: Cell<usize>
}

impl Node {
    /// Create a new node that has recently responded to us but never requested from us.
    pub fn as_good(id: NodeId, addr: SocketAddr) -> Node {
        Node{ id: id, addr: addr, last_response: Cell::new(Some(UTC::now())),
            last_request: Cell::new(None), refresh_requests: Cell::new(0) }
    }
    
    /// Create a questionable node that has responded to us before but never requested from us.
    pub fn as_questionable(id: NodeId, addr: SocketAddr) -> Node {
        let last_response_offset = Duration::minutes(MAX_LAST_SEEN_MINS);
        let last_response = test::travel_into_past(last_response_offset);
        
        Node{ id: id, addr: addr, last_response: Cell::new(Some(last_response)),
            last_request: Cell::new(None), refresh_requests: Cell::new(0) }
    }
    
    /// Create a new node that has never responded to us or requested from us.
    pub fn as_bad(id: NodeId, addr: SocketAddr) -> Node {
        Node{ id: id, addr: addr, last_response: Cell::new(None),
            last_request: Cell::new(None), refresh_requests: Cell::new(0) }
    }
    
    /// Record that we sent the node a request.
    pub fn local_request(&self) {
        if self.status() != NodeStatus::Good {
            let num_requests = self.refresh_requests.get() + 1;
            
            self.refresh_requests.set(num_requests);
        }
    }
    
    /// Record that the node sent us a request.
    pub fn remote_request(&self) {
        self.last_request.set(Some(UTC::now()));
    }
    
    /// Record that the node sent us a response.
    pub fn remote_response(&self) {
        self.last_response.set(Some(UTC::now()));
        
        self.refresh_requests.set(0);
    }
    
    pub fn id(&self) -> NodeId {
        self.id
    }
    
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
    
    /// Current status of the node.
    pub fn status(&self) -> NodeStatus {
        let curr_time = UTC::now();
        
        match recently_responded(self, curr_time) {
            NodeStatus::Good         => return NodeStatus::Good,
            NodeStatus::Bad          => return NodeStatus::Bad,
            NodeStatus::Questionable => ()
        };
        
        recently_requested(self, curr_time)
    }
}

impl Eq for Node { }

impl PartialEq<Node> for Node {
    fn eq(&self, other: &Node) -> bool {
        self.id == other.id && self.addr == other.addr
    }
}

impl Clone for Node {
    fn clone(&self) -> Node {
        Node{ id: self.id, addr: self.addr, last_response: self.last_response.clone(),
            last_request: self.last_request.clone(), refresh_requests: self.refresh_requests.clone() }
    }
}

/// First scenario where a node is good is if it has responded to one of our requests recently.
///
/// Returns the status of the node where a Questionable status means the node has responded
/// to us before, but not recently.
fn recently_responded(node: &Node, curr_time: DateTime<UTC>) -> NodeStatus {
    // Check if node has ever responded to us
    let since_response = match node.last_response.get() {
        Some(response_time) => curr_time - response_time,
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
fn recently_requested(node: &Node, curr_time: DateTime<UTC>) -> NodeStatus {
    let max_last_request = Duration::minutes(MAX_LAST_SEEN_MINS);

    // Check if the node has recently request from us
    if let Some(request_time) = node.last_request.get() {
        let since_request = curr_time - request_time;
        
        if since_request < max_last_request {
            return NodeStatus::Good
        }
    }
    
    // Check if we have request from node multiple times already without response
    if node.refresh_requests.get() < MAX_REFRESH_REQUESTS {
        NodeStatus::Questionable
    } else {
        NodeStatus::Bad
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
    use chrono::{Duration};
    
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