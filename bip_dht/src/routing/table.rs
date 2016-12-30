// TODO: Remove when we use find_node,
#![allow(unused)]

use std::iter::Filter;
use std::slice::Iter;

use bip_util::bt::NodeId;
use bip_util::sha::{self, ShaHash, XorRep};
use rand;

use routing::bucket::{self, Bucket};
use routing::node::{Node, NodeStatus};

pub const MAX_BUCKETS: usize = sha::SHA_HASH_LEN * 8;

/// Routing table containing a table of routing nodes as well
/// as the id of the local node participating in the dht.
pub struct RoutingTable {
    // Important: Our node id will always fall within the range
    // of the last bucket in the buckets array.
    buckets: Vec<Bucket>,
    node_id: NodeId,
}

impl RoutingTable {
    /// Create a new RoutingTable with the given node id as our id.
    pub fn new(node_id: NodeId) -> RoutingTable {
        let buckets = vec![Bucket::new()];

        RoutingTable {
            buckets: buckets,
            node_id: node_id,
        }
    }

    /// Return the node id of the RoutingTable.
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Iterator over the closest good nodes to the given node id.
    ///
    /// The closeness of nodes has a maximum granularity of a bucket. For most use
    /// cases this is fine since we will usually be performing lookups and aggregating
    /// a number of results equal to the size of a bucket.
    pub fn closest_nodes<'a>(&'a self, node_id: NodeId) -> ClosestNodes<'a> {
        ClosestNodes::new(&self.buckets, self.node_id, node_id)
    }

    /// Iterator over all buckets in the routing table.
    pub fn buckets<'a>(&'a self) -> Buckets<'a> {
        Buckets::new(&self.buckets)
    }

    /// Find an instance of the target node in the RoutingTable, if it exists.
    pub fn find_node(&self, node: &Node) -> Option<&Node> {
        let bucket_index = leading_bit_count(self.node_id, node.id());

        // Check the sorted bucket
        let opt_bucket_contents = if let Some(c) = self.buckets().skip(bucket_index).next() {
            // Got the sorted bucket
            Some(c)
        } else {
            // Grab the assorted bucket (if it exists)
            self.buckets().find(|c| {
                match c {
                    &BucketContents::Empty => false,
                    &BucketContents::Sorted(_) => false,
                    &BucketContents::Assorted(_) => true,
                }
            })
        };

        // Check for our target node in our results
        match opt_bucket_contents {
            Some(BucketContents::Sorted(b)) => b.pingable_nodes().find(|n| n == &node),
            Some(BucketContents::Assorted(b)) => b.pingable_nodes().find(|n| n == &node),
            _ => None,
        }
    }

    /// Add the node to the RoutingTable if there is space for it.
    pub fn add_node(&mut self, node: Node) {
        // Doing some checks and calculations here, outside of the recursion
        if node.status() == NodeStatus::Bad {
            return;
        }
        let num_same_bits = leading_bit_count(self.node_id, node.id());

        // Should not add a node that has the same id as us
        if num_same_bits != MAX_BUCKETS {
            self.bucket_node(node, num_same_bits);
        }
    }

    /// Recursively tries to place the node into some bucket.
    fn bucket_node(&mut self, node: Node, num_same_bits: usize) {
        let bucket_index = bucket_placement(num_same_bits, self.buckets.len());

        // Try to place in correct bucket
        if !self.buckets[bucket_index].add_node(node.clone()) {
            // Bucket was full, try to split it
            if self.split_bucket(bucket_index) {
                // Bucket split successfully, try to add again
                self.bucket_node(node.clone(), num_same_bits);
            }
        }
    }

    /// Tries to split the bucket at the specified index.
    ///
    /// Returns false if the split cannot be performed.
    fn split_bucket(&mut self, bucket_index: usize) -> bool {
        if !can_split_bucket(self.buckets.len(), bucket_index) {
            return false;
        }

        // Implementation is easier if we just remove the whole bucket, pretty
        // cheap to copy and we can manipulate the new buckets while they are
        // in the RoutingTable already.
        let split_bucket = match self.buckets.pop() {
            Some(bucket) => bucket,
            None => panic!("No buckets present in RoutingTable, implementation error..."),
        };

        // Push two more buckets to distribute nodes between
        self.buckets.push(Bucket::new());
        self.buckets.push(Bucket::new());

        for node in split_bucket.iter() {
            self.add_node(node.clone());
        }

        true
    }
}

/// Returns true if the bucket can be split.
fn can_split_bucket(num_buckets: usize, bucket_index: usize) -> bool {
    bucket_index == num_buckets - 1 && bucket_index != MAX_BUCKETS - 1
}

/// Generates a random NodeId.
///
/// TODO: Shouldnt use this in the future to get an id for the routing table,
/// generate one from the security module to be compliant with the spec.
pub fn random_node_id() -> NodeId {
    let mut random_sha_hash = [0u8; sha::SHA_HASH_LEN];

    for byte in random_sha_hash.iter_mut() {
        *byte = rand::random::<u8>();
    }

    ShaHash::from(random_sha_hash)
}

/// Number of leading bits that are identical between the local and remote node ids.
pub fn leading_bit_count(local_node: NodeId, remote_node: NodeId) -> usize {
    let diff_id = local_node ^ remote_node;

    diff_id.bits().take_while(|&x| x == XorRep::Same).count()
}

/// Take the number of leading bits that are the same between our node and the remote
/// node and calculate a bucket index for that node id.
fn bucket_placement(num_same_bits: usize, num_buckets: usize) -> usize {
    // The index that the node should be placed in *eventually*, meaning
    // when we create enough buckets for that bucket to appear.
    let ideal_index = num_same_bits;

    if ideal_index >= num_buckets {
        num_buckets - 1
    } else {
        ideal_index
    }
}

// ----------------------------------------------------------------------------//

#[derive(Copy, Clone)]
pub enum BucketContents<'a> {
    /// Empty bucket is a placeholder for a bucket that has not yet been created.
    Empty,
    /// Sorted bucket is where nodes with the same leading bits reside.
    Sorted(&'a Bucket),
    /// Assorted bucket is where nodes with differing bits reside.
    ///
    /// These nodes are dynamically placed in their sorted bucket when is is created.
    Assorted(&'a Bucket),
}

impl<'a> BucketContents<'a> {
    fn is_empty(&self) -> bool {
        match self {
            &BucketContents::Empty => true,
            _ => false,
        }
    }

    fn is_sorted(&self) -> bool {
        match self {
            &BucketContents::Sorted(_) => true,
            _ => false,
        }
    }

    fn is_assorted(&self) -> bool {
        match self {
            &BucketContents::Assorted(_) => true,
            _ => false,
        }
    }
}

/// Iterator over buckets where the item returned is an enum
/// specifying the current state of the bucket returned.
#[derive(Copy, Clone)]
pub struct Buckets<'a> {
    buckets: &'a [Bucket],
    index: usize,
}

impl<'a> Buckets<'a> {
    fn new(buckets: &'a [Bucket]) -> Buckets<'a> {
        Buckets {
            buckets: buckets,
            index: 0,
        }
    }
}

impl<'a> Iterator for Buckets<'a> {
    type Item = BucketContents<'a>;

    fn next(&mut self) -> Option<BucketContents<'a>> {
        if self.index > MAX_BUCKETS {
            return None;
        } else if self.index == MAX_BUCKETS {
            // If not all sorted buckets were present, return the assorted bucket
            // after the iteration of the last bucket occurs, which is here!
            self.index += 1;

            return if self.buckets.len() == MAX_BUCKETS || self.buckets.is_empty() {
                None
            } else {
                Some(BucketContents::Assorted(&self.buckets[self.buckets.len() - 1]))
            };
        }

        if self.index + 1 < self.buckets.len() || self.buckets.len() == MAX_BUCKETS {
            self.index += 1;

            Some(BucketContents::Sorted(&self.buckets[self.index - 1]))
        } else {
            self.index += 1;

            Some(BucketContents::Empty)
        }
    }
}

// ----------------------------------------------------------------------------//

// Iterator filter for only good nodes.
type GoodNodes<'a> = Filter<Iter<'a, Node>, fn(&&Node) -> bool>;

// So what we are going to do here is iterate over every bucket in a hypothetically filled
// routing table (buckets slice). If the bucket we are interested in has not been created
// yet (not in the slice), go through the last bucket (assorted nodes) and check if any nodes
// would have been placed in that bucket. If we find one, return it and mark it in our assorted
// nodes array.
pub struct ClosestNodes<'a> {
    buckets: &'a [Bucket],
    current_iter: Option<GoodNodes<'a>>,
    current_index: usize,
    start_index: usize,
    // Since we could have assorted nodes that are interleaved between our sorted
    // nodes as far as closest nodes are concerned, we need some way to hand the
    // assorted nodes out and keep track of which ones we have handed out.
    // (Bucket Index, Node Reference, Returned Before)
    assorted_nodes: Option<[(usize, &'a Node, bool); bucket::MAX_BUCKET_SIZE]>,
}

impl<'a> ClosestNodes<'a> {
    fn new(buckets: &'a [Bucket], self_node_id: NodeId, other_node_id: NodeId) -> ClosestNodes<'a> {
        let start_index = leading_bit_count(self_node_id, other_node_id);

        let current_iter = bucket_iterator(buckets, start_index);
        let assorted_nodes = precompute_assorted_nodes(buckets, self_node_id);

        ClosestNodes {
            buckets: buckets,
            current_iter: current_iter,
            current_index: start_index,
            start_index: start_index,
            assorted_nodes: assorted_nodes,
        }
    }
}

impl<'a> Iterator for ClosestNodes<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<&'a Node> {
        let current_index = self.current_index;

        // Check if we have any nodes left in the current iterator
        if let Some(ref mut iter) = self.current_iter {
            match iter.next() {
                Some(node) => return Some(node),
                None => (),
            };
        }

        // Check if we have any nodes to give in the assorted bucket
        if let Some(ref mut nodes) = self.assorted_nodes {
            let mut nodes_iter = nodes.iter_mut().filter(|tup| is_good_node(&tup.1));

            match nodes_iter.find(|tup| tup.0 == current_index && !tup.2) {
                Some(node) => {
                    node.2 = true;

                    return Some(node.1);
                }
                None => (),
            };
        }

        // Check if we can move to a new bucket
        match next_bucket_index(MAX_BUCKETS, self.start_index, self.current_index) {
            Some(new_index) => {
                self.current_index = new_index;
                self.current_iter = bucket_iterator(self.buckets, self.current_index);

                // Recurse back into this function to check the previous code paths again
                self.next()
            }
            None => None,
        }
    }
}

/// Optionally returns the precomputed bucket positions for all assorted nodes.
fn precompute_assorted_nodes<'a>(buckets: &'a [Bucket],
                                 self_node_id: NodeId)
                                 -> Option<[(usize, &'a Node, bool); bucket::MAX_BUCKET_SIZE]> {
    if buckets.len() == MAX_BUCKETS {
        return None;
    }
    let assorted_bucket = &buckets[buckets.len() - 1];
    let mut assorted_iter = assorted_bucket.iter().peekable();

    // So the bucket is not empty and now we have a reference to initialize our stack allocated array.
    if let Some(&init_reference) = assorted_iter.peek() {
        // Set all tuples to true in case our bucket is not full.
        let mut assorted_nodes = [(0, init_reference, true); bucket::MAX_BUCKET_SIZE];

        for (index, node) in assorted_iter.enumerate() {
            let bucket_index = leading_bit_count(self_node_id, node.id());

            assorted_nodes[index] = (bucket_index, node, false);
        }

        Some(assorted_nodes)
    } else {
        None
    }
}

/// Optionally returns the filter iterator for the bucket at the specified index.
fn bucket_iterator<'a>(buckets: &'a [Bucket], index: usize) -> Option<GoodNodes<'a>> {
    if buckets.len() == MAX_BUCKETS {
            buckets
        } else {
            &buckets[..(buckets.len() - 1)]
        }
        .get(index)
        .map(|bucket| good_node_filter(bucket.iter()))
}

/// Converts the given iterator into a filter iterator to return only good nodes.
fn good_node_filter<'a>(iter: Iter<'a, Node>) -> GoodNodes<'a> {
    iter.filter(is_good_node)
}

/// Shakes fist at iterator making me take a double reference (could avoid it by mapping, but oh well)
fn is_good_node(node: &&Node) -> bool {
    let status = node.status();

    status == NodeStatus::Good || status == NodeStatus::Questionable
}

/// Computes the next bucket index that should be visited given the number of buckets, the starting index
/// and the current index.
///
/// Returns None if all of the buckets have been visited.
fn next_bucket_index(num_buckets: usize, start_index: usize, curr_index: usize) -> Option<usize> {
    // Since we prefer going right first, that means if we are on the right side then we want to go
    // to the same offset on the left, however, if we are on the left we want to go 1 past the offset
    // to the right. All assuming we can actually do this without going out of bounds.
    if curr_index == start_index {
        let right_index = start_index.checked_add(1);
        let left_index = start_index.checked_sub(1);

        if index_is_in_bounds(num_buckets, right_index) {
            Some(right_index.unwrap())
        } else if index_is_in_bounds(num_buckets, left_index) {
            Some(left_index.unwrap())
        } else {
            None
        }
    } else if curr_index > start_index {
        let offset = curr_index - start_index;

        let left_index = start_index.checked_sub(offset);
        let right_index = curr_index.checked_add(1);

        if index_is_in_bounds(num_buckets, left_index) {
            Some(left_index.unwrap())
        } else if index_is_in_bounds(num_buckets, right_index) {
            Some(right_index.unwrap())
        } else {
            None
        }
    } else {
        let offset = (start_index - curr_index) + 1;

        let right_index = start_index.checked_add(offset);
        let left_index = curr_index.checked_sub(1);

        if index_is_in_bounds(num_buckets, right_index) {
            Some(right_index.unwrap())
        } else if index_is_in_bounds(num_buckets, left_index) {
            Some(left_index.unwrap())
        } else {
            None
        }
    }
}

/// Returns true if the overflow checked index is in bounds of the given length.
fn index_is_in_bounds(length: usize, checked_index: Option<usize>) -> bool {
    match checked_index {
        Some(index) => index < length,
        None => false,
    }
}

// ----------------------------------------------------------------------------//

#[cfg(test)]
mod tests {
    use bip_util::bt::{self, NodeId};
    use bip_util::test as bip_test;

    use routing::table::{self, RoutingTable, BucketContents};
    use routing::bucket;
    use routing::node::Node;

    // TODO: Move into bip_util crate
    fn flip_id_bit_at_index(node_id: NodeId, index: usize) -> NodeId {
        let mut id_bytes: [u8; bt::NODE_ID_LEN] = node_id.into();
        let (byte_index, bit_index) = (index / 8, index % 8);

        let actual_bit_index = 7 - bit_index;
        id_bytes[byte_index] ^= 1 << actual_bit_index;

        id_bytes.into()
    }

    #[test]
    fn positive_add_node_max_recursion() {
        let table_id = [1u8; bt::NODE_ID_LEN];
        let mut table = RoutingTable::new(table_id.into());

        let mut node_id = table_id;
        // Modify the id so it is placed in the last bucket
        node_id[bt::NODE_ID_LEN - 1] = 0;

        // Trigger a bucket overflow and since the ids are placed in the last bucket, all of
        // the buckets will be recursively created and inserted into the list of all buckets.
        let block_addrs = bip_test::dummy_block_socket_addrs((bucket::MAX_BUCKET_SIZE + 1) as u16);
        for index in 0..(bucket::MAX_BUCKET_SIZE + 1) {
            let node = Node::as_good(node_id.into(), block_addrs[index]);

            table.add_node(node);
        }
    }

    #[test]
    fn positive_initial_empty_buckets() {
        let table_id = [1u8; bt::NODE_ID_LEN];
        let mut table = RoutingTable::new(table_id.into());

        // First buckets should be empty
        assert_eq!(table.buckets().take(table::MAX_BUCKETS).count(),
                   table::MAX_BUCKETS);
        assert!(table.buckets()
            .take(table::MAX_BUCKETS)
            .fold(true, |prev, contents| prev && contents.is_empty()));

        // Last assorted bucket should show up
        assert_eq!(table.buckets().skip(table::MAX_BUCKETS).count(), 1);
        for bucket in table.buckets().skip(table::MAX_BUCKETS) {
            match bucket {
                BucketContents::Assorted(b) => assert_eq!(b.pingable_nodes().count(), 0),
                _ => panic!("Expected BucketContents::Assorted"),
            }
        }
    }

    #[test]
    fn positive_first_bucket_sorted() {
        let table_id = [1u8; bt::NODE_ID_LEN];
        let mut table = RoutingTable::new(table_id.into());

        let mut node_id = table_id;
        // Flip first bit so we are placed in the first bucket
        node_id[0] |= 128;

        let block_addrs = bip_test::dummy_block_socket_addrs((bucket::MAX_BUCKET_SIZE + 1) as u16);
        for index in 0..(bucket::MAX_BUCKET_SIZE + 1) {
            let node = Node::as_good(node_id.into(), block_addrs[index]);

            table.add_node(node);
        }

        // First bucket should be sorted
        assert_eq!(table.buckets().take(1).count(), 1);
        for bucket in table.buckets().take(1) {
            match bucket {
                BucketContents::Sorted(b) => {
                    assert_eq!(b.pingable_nodes().count(), bucket::MAX_BUCKET_SIZE)
                }
                _ => panic!("Expected BucketContents::Sorted"),
            }
        }

        // Middle buckets should be empty
        assert_eq!(table.buckets().skip(1).take(table::MAX_BUCKETS - 1).count(),
                   table::MAX_BUCKETS - 1);
        assert!(table.buckets()
            .skip(1)
            .take(table::MAX_BUCKETS - 1)
            .fold(true, |prev, contents| prev && contents.is_empty()));

        // Last assorted bucket should show up
        assert_eq!(table.buckets().skip(table::MAX_BUCKETS).count(), 1);
        for bucket in table.buckets().skip(table::MAX_BUCKETS) {
            match bucket {
                BucketContents::Assorted(b) => assert_eq!(b.pingable_nodes().count(), 0),
                _ => panic!("Expected BucketContents::Assorted"),
            }
        }
    }

    #[test]
    fn positive_last_bucket_sorted() {
        let table_id = [1u8; bt::NODE_ID_LEN];
        let mut table = RoutingTable::new(table_id.into());

        let mut node_id = table_id;
        // Flip last bit so we are placed in the last bucket
        node_id[bt::NODE_ID_LEN - 1] = 0;

        let block_addrs = bip_test::dummy_block_socket_addrs((bucket::MAX_BUCKET_SIZE + 1) as u16);
        for index in 0..(bucket::MAX_BUCKET_SIZE + 1) {
            let node = Node::as_good(node_id.into(), block_addrs[index]);

            table.add_node(node);
        }

        // First buckets should be sorted (although they are all empty)
        assert_eq!(table.buckets().take(table::MAX_BUCKETS - 1).count(),
                   table::MAX_BUCKETS - 1);
        for bucket in table.buckets().take(table::MAX_BUCKETS - 1) {
            match bucket {
                BucketContents::Sorted(b) => assert_eq!(b.pingable_nodes().count(), 0),
                _ => panic!("Expected BucketContents::Sorted"),
            }
        }

        // Last bucket should be sorted
        assert_eq!(table.buckets().skip(table::MAX_BUCKETS - 1).take(1).count(),
                   1);
        for bucket in table.buckets().skip(table::MAX_BUCKETS - 1).take(1) {
            match bucket {
                BucketContents::Sorted(b) => {
                    assert_eq!(b.pingable_nodes().count(), bucket::MAX_BUCKET_SIZE)
                }
                _ => panic!("Expected BucketContents::Sorted"),
            }
        }

        // Last assorted bucket should NOT show up
        assert_eq!(table.buckets().skip(table::MAX_BUCKETS).count(), 0);
    }

    #[test]
    fn positive_all_sorted_buckets() {
        let table_id = [1u8; bt::NODE_ID_LEN];
        let mut table = RoutingTable::new(table_id.into());

        let block_addrs = bip_test::dummy_block_socket_addrs(bucket::MAX_BUCKET_SIZE as u16);
        for bit_flip_index in 0..table::MAX_BUCKETS {
            for addr_index in 0..block_addrs.len() {
                let bucket_node_id = flip_id_bit_at_index(table_id.into(), bit_flip_index);

                table.add_node(Node::as_good(bucket_node_id, block_addrs[addr_index]));
            }
        }

        assert_eq!(table.buckets().count(), table::MAX_BUCKETS);
        for bucket in table.buckets() {
            match bucket {
                BucketContents::Sorted(b) => {
                    assert_eq!(b.pingable_nodes().count(), bucket::MAX_BUCKET_SIZE)
                }
                _ => panic!("Expected BucketContents::Sorted"),
            }
        }
    }

    #[test]
    fn negative_node_id_equal_table_id() {
        let table_id = [1u8; bt::NODE_ID_LEN];
        let mut table = RoutingTable::new(table_id.into());

        assert_eq!(table.closest_nodes(table_id.into()).count(), 0);

        let node = Node::as_good(table_id.into(), bip_test::dummy_socket_addr_v4());
        table.add_node(node);

        assert_eq!(table.closest_nodes(table_id.into()).count(), 0);
    }
}
