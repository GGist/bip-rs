// TODO: Remove when the routing table uses the new bucket iterators.
#![allow(unused)]

use std::iter::Filter;
use std::net::{Ipv4Addr, SocketAddrV4, SocketAddr};
use std::slice::Iter;

use bip_util::bt::{self, NodeId};

use routing::node::{Node, NodeStatus};

/// Maximum number of nodes that should reside in any bucket.
pub const MAX_BUCKET_SIZE: usize = 8;

/// Bucket containing Nodes with identical bit prefixes.
pub struct Bucket {
    nodes: [Node; MAX_BUCKET_SIZE],
}

impl Bucket {
    /// Create a new Bucket with all Nodes default initialized.
    pub fn new() -> Bucket {
        let id = NodeId::from([0u8; bt::NODE_ID_LEN]);

        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));

        Bucket {
            nodes: [Node::as_bad(id, addr),
                    Node::as_bad(id, addr),
                    Node::as_bad(id, addr),
                    Node::as_bad(id, addr),
                    Node::as_bad(id, addr),
                    Node::as_bad(id, addr),
                    Node::as_bad(id, addr),
                    Node::as_bad(id, addr)],
        }
    }

    /// Iterator over all good nodes in the bucket.
    pub fn good_nodes<'a>(&'a self) -> GoodNodes<'a> {
        GoodNodes::new(&self.nodes)
    }

    /// Iterator over all good nodes and questionable nodes in the bucket.
    pub fn pingable_nodes<'a>(&'a self) -> PingableNodes<'a> {
        PingableNodes::new(&self.nodes)
    }

    /// Iterator over each node within the bucket.
    ///
    /// For buckets newly created, the initial bad nodes are included.
    pub fn iter(&self) -> Iter<Node> {
        self.nodes.iter()
    }

    /// Indicates if the bucket needs to be refreshed.
    pub fn needs_refresh(&self) -> bool {
        self.nodes.iter().fold(true, |prev, node| prev && node.status() != NodeStatus::Good)
    }

    /// Attempt to add the given Node to the bucket if it is not in a bad state.
    ///
    /// Returns false if the Node could not be placed in the bucket because it is full.
    pub fn add_node(&mut self, new_node: Node) -> bool {
        let new_node_status = new_node.status();
        if new_node_status == NodeStatus::Bad {
            return true;
        }

        // See if this node is already in the table, in that case replace it if it
        // has a higher or equal status to the current node.
        if let Some(index) = self.nodes.iter().position(|node| *node == new_node) {
            let other_node_status = self.nodes[index].status();

            if new_node_status >= other_node_status {
                self.nodes[index] = new_node;
            }

            return true;
        }

        // See if any lower priority nodes are present in the table, we cant do
        // nodes that have equal status because we have to prefer longer lasting
        // nodes in the case of a good status which helps with stability.
        let replace_index = self.nodes.iter().position(|node| node.status() < new_node_status);
        if let Some(index) = replace_index {
            self.nodes[index] = new_node;

            true
        } else {
            false
        }
    }
}

// ----------------------------------------------------------------------------//

pub struct GoodNodes<'a> {
    iter: Filter<Iter<'a, Node>, fn(&&Node) -> bool>,
}

impl<'a> GoodNodes<'a> {
    fn new(nodes: &'a [Node]) -> GoodNodes<'a> {
        GoodNodes { iter: nodes.iter().filter(good_nodes_filter) }
    }
}

fn good_nodes_filter(node: &&Node) -> bool {
    node.status() == NodeStatus::Good
}

impl<'a> Iterator for GoodNodes<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<&'a Node> {
        self.iter.next()
    }
}

// ----------------------------------------------------------------------------//

pub struct PingableNodes<'a> {
    iter: Filter<Iter<'a, Node>, fn(&&Node) -> bool>,
}

impl<'a> PingableNodes<'a> {
    fn new(nodes: &'a [Node]) -> PingableNodes<'a> {
        PingableNodes { iter: nodes.iter().filter(pingable_nodes_filter) }
    }
}

fn pingable_nodes_filter(node: &&Node) -> bool {
    // Function is moderately expensive
    let status = node.status();

    status == NodeStatus::Good || status == NodeStatus::Questionable
}

impl<'a> Iterator for PingableNodes<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<&'a Node> {
        self.iter.next()
    }
}

// ----------------------------------------------------------------------------//

#[cfg(test)]
mod tests {
    use bip_util::sha::{self, ShaHash};
    use bip_util::test as bip_test;

    use routing::bucket::{self, Bucket};
    use routing::node::{Node, NodeStatus};

    #[test]
    fn positive_initial_no_nodes() {
        let bucket = Bucket::new();

        assert_eq!(bucket.good_nodes().count(), 0);
        assert_eq!(bucket.pingable_nodes().count(), 0);
    }

    #[test]
    fn positive_all_questionable_nodes() {
        let mut bucket = Bucket::new();

        let dummy_addr = bip_test::dummy_socket_addr_v4();
        let dummy_ids = bip_test::dummy_block_node_ids(super::MAX_BUCKET_SIZE as u8);
        for index in 0..super::MAX_BUCKET_SIZE {
            let node = Node::as_questionable(dummy_ids[index], dummy_addr);
            bucket.add_node(node);
        }

        assert_eq!(bucket.good_nodes().count(), 0);
        assert_eq!(bucket.pingable_nodes().count(), super::MAX_BUCKET_SIZE);
    }

    #[test]
    fn positive_all_good_nodes() {
        let mut bucket = Bucket::new();

        let dummy_addr = bip_test::dummy_socket_addr_v4();
        let dummy_ids = bip_test::dummy_block_node_ids(super::MAX_BUCKET_SIZE as u8);
        for index in 0..super::MAX_BUCKET_SIZE {
            let node = Node::as_good(dummy_ids[index], dummy_addr);
            bucket.add_node(node);
        }

        assert_eq!(bucket.good_nodes().count(), super::MAX_BUCKET_SIZE);
        assert_eq!(bucket.pingable_nodes().count(), super::MAX_BUCKET_SIZE);
    }

    #[test]
    fn positive_replace_questionable_node() {
        let mut bucket = Bucket::new();

        let dummy_addr = bip_test::dummy_socket_addr_v4();
        let dummy_ids = bip_test::dummy_block_node_ids(super::MAX_BUCKET_SIZE as u8);
        for index in 0..super::MAX_BUCKET_SIZE {
            let node = Node::as_questionable(dummy_ids[index], dummy_addr);
            bucket.add_node(node);
        }

        assert_eq!(bucket.good_nodes().count(), 0);
        assert_eq!(bucket.pingable_nodes().count(), super::MAX_BUCKET_SIZE);

        let good_node = Node::as_good(dummy_ids[0], dummy_addr);
        bucket.add_node(good_node.clone());

        assert_eq!(bucket.good_nodes().next().unwrap(), &good_node);
        assert_eq!(bucket.good_nodes().count(), 1);
        assert_eq!(bucket.pingable_nodes().count(), super::MAX_BUCKET_SIZE);
    }

    #[test]
    fn positive_resist_good_node_churn() {
        let mut bucket = Bucket::new();

        let dummy_addr = bip_test::dummy_socket_addr_v4();
        let dummy_ids = bip_test::dummy_block_node_ids((super::MAX_BUCKET_SIZE as u8) + 1);
        for index in 0..super::MAX_BUCKET_SIZE {
            let node = Node::as_good(dummy_ids[index], dummy_addr);
            bucket.add_node(node);
        }

        // All the nodes should be good
        assert_eq!(bucket.good_nodes().count(), super::MAX_BUCKET_SIZE);

        // Create a new good node
        let unused_id = dummy_ids[dummy_ids.len() - 1];
        let new_good_node = Node::as_good(unused_id, dummy_addr);

        // Make sure the node is NOT in the bucket
        assert!(bucket.good_nodes().find(|node| &&new_good_node == node).is_none());

        // Try to add it
        bucket.add_node(new_good_node.clone());

        // Make sure the node is NOT in the bucket
        assert!(bucket.good_nodes().find(|node| &&new_good_node == node).is_none());
    }

    #[test]
    fn positive_resist_questionable_node_churn() {
        let mut bucket = Bucket::new();

        let dummy_addr = bip_test::dummy_socket_addr_v4();
        let dummy_ids = bip_test::dummy_block_node_ids((super::MAX_BUCKET_SIZE as u8) + 1);
        for index in 0..super::MAX_BUCKET_SIZE {
            let node = Node::as_questionable(dummy_ids[index], dummy_addr);
            bucket.add_node(node);
        }

        // All the nodes should be questionable
        assert_eq!(bucket.pingable_nodes()
                       .filter(|node| node.status() == NodeStatus::Questionable)
                       .count(),
                   super::MAX_BUCKET_SIZE);

        // Create a new questionable node
        let unused_id = dummy_ids[dummy_ids.len() - 1];
        let new_questionable_node = Node::as_questionable(unused_id, dummy_addr);

        // Make sure the node is NOT in the bucket
        assert!(bucket.pingable_nodes().find(|node| &&new_questionable_node == node).is_none());

        // Try to add it
        bucket.add_node(new_questionable_node.clone());

        // Make sure the node is NOT in the bucket
        assert_eq!(bucket.pingable_nodes()
                       .filter(|node| node.status() == NodeStatus::Questionable)
                       .count(),
                   super::MAX_BUCKET_SIZE);
    }
}
