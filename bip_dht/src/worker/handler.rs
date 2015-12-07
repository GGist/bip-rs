use std::cell::{Cell};
use std::collections::{HashSet};
use std::convert::{AsRef};
use std::io::{self};
use std::iter::{self};
use std::net::{SocketAddr};
use std::sync::{Arc};
use std::sync::mpsc::{SyncSender};
use std::thread::{self};

use bip_bencode::{Bencode};
use bip_util::{self, NodeId, InfoHash};
use mio::{EventLoop, Sender, Timeout, Handler};

use message::{MessageType};
use message::find_node::{FindNodeRequest};
use message::response::{ResponseType, ExpectedResponse};
use router::{Router};
use routing::bucket::{Bucket};
use routing::node::{Node};
use routing::table::{self, RoutingTable};
use worker::{self, OneshotTask, ScheduledTask};
use worker::lookup::{HashLookup, LookupResult};

use routing::table::{BucketContents};
use routing::node::{NodeStatus};

pub fn create_dht_handler<H>(table: RoutingTable, out: SyncSender<(Vec<u8>, SocketAddr)>, handshaker: H)
    -> io::Result<Sender<OneshotTask>> where H: Handshaker {
    let mut handler = DhtHandler::new(table, out);
    let mut event_loop = try!(EventLoop::new());
    
    let loop_channel = event_loop.channel();
    
    thread::spawn(move || event_loop.run(&mut handler));
    
    Ok(loop_channel)
}

//----------------------------------------------------------------------------//

pub struct DhtHandler<H> where H: Handshaker {
    handshaker:     H,
    out_channel:    SyncSender<(Vec<u8>, SocketAddr)>,
    boostrapper:    (ActionID, TableBootstrap),
    token_store:    TokenStore,
    aid_generator:  AIDGenerator,
    routing_table:  RoutingTable,
    active_lookups: HashMap<ActionID, (TableLookup, SyncSender<()>)>
}

impl<H> DhtHandler<H> {
    fn new(table: RoutingTable, out: SyncSender<(Vec<u8>, SocketAddr)>, handshaker: H) -> DhtHandler {
        
    
    
    
        DhtHandler{ handshaker: handshaker, out_channel: out, 
        
        
        routing_table: table, current_routers: HashSet::new(),
            current_activites: HashMap::new() }
    }
}

impl<H> Handler for DhtHandler<H> {
    type Timeout = (u64, IntervalTask);
    type Message = OneshotTask;
    
    fn notify(&mut self, event_loop: &mut EventLoop<DhtHandler>, task: OneshotTask) {
        match task {
            OneshotTask::Incoming(buffer, addr) => {
                handle_incoming(self, event_loop, &buffer[..], addr);
            },
            OneshotTask::ScheduleTask(timeout, task) => {
                handle_schedule_task(event_loop, timeout, task);
            },
            OneshotTask::StartBootstrap(routers, nodes) => {
                handle_start_bootstrap(self, &routers[..], &nodes[..]);
            },
            OneshotTask::StartLookup(info_hash, sender) => {
                handle_start_lookup(self, event_loop, info_hash, sender);
            }
        }
    }
    
    fn timeout(&mut self, event_loop: &mut EventLoop<DhtHandler>, data: (u64, IntervalTask)) {
        let (timeout, task) = data;
        
        match task {
            ScheduledTask::CheckTableRefresh(bucket_index) => {
                handle_check_table_refresh(self, bucket_index);
            },
            ScheduledTask::CheckBootstrapTimeout(trans_id, node) => {
                handle_check_bootstrap_timeout(self, event_loop, trans_id, node);
            },
            ScheduledTask::CheckLookupTimeout(trans_id, Node) => {
                handle_check_lookup_timeout(self, event_loop, trans_id, node);
            },
            ScheduledTask::CheckLookupEndGame(action_id) => {
                handle_check_lookup_endgame(self, action_id)
            }
        }
    }
}

//----------------------------------------------------------------------------//

fn handle_incoming(handler: &mut DhtHandler, event_loop: &mut EventLoop<DhtHandler>, buffer: &[u8], addr: SocketAddr) {
    let bencode = if let Ok(b) = Bencode::decode(buffer) {
        b
    } else {
        warn!("bip_dht: Received invalid bencode data...");
        return
    };
    
    let message = MessageType::new(&bencode, |trans| {
        if trans[0] == 48 {
            ExpectedResponse::FindNode
        } else {
            ExpectedResponse::GetPeers
        }
    });
    match message {
        Ok(MessageType::Response(ResponseType::FindNode(f))) => {
            // Add returned nodes as questionable (unpinged in this case)
            for (node_id, v4_addr) in f.nodes() {
                let addr = SocketAddr::V4(v4_addr);
                let node = Node::as_questionable(node_id, addr);
                
                handler.routing_table.add_node(node);
            }
            
            // Add responding node as good
            if !handler.current_routers.contains(&addr) {
                let node = Node::as_good(f.node_id(), addr);
                
                println!("Responding Node: {}", addr);
                
                handler.routing_table.add_node(node);
            }
        },
        Ok(MessageType::Response(ResponseType::GetPeers(g))) => {
            // Update our routing table
            // TODO: ^^^
            println!("NODE RESPONSE");
            let node = Node::as_good(g.node_id(), addr);
            if !handler.active_lookups.is_empty() {
                println!("{:?}", handler.active_lookups[0].node_response(node, g, &handler.out_channel, event_loop));
            }
        }
        _ => warn!("bip_dht: Received unsupported message... {:?}", message)
    };
}

//----------------------------------------------------------------------------//

fn handle_start_bootstrap(handler: &mut DhtHandler, routers: &[Router], nodes: &[SocketAddr]) {
    let node_id = handler.routing_table.node_id();
    let router_filter = routers.iter().filter_map(|r| r.ipv4_addr().ok().map(|m| SocketAddr::V4(m)) ).collect::<HashSet<SocketAddr>>();
    
    let find_node = FindNodeRequest::new(&b"0"[..], node_id, node_id);
    let find_node_message = find_node.encode();
    
    // Send messages to all routers
    for addr in router_filter.iter() {
        if handler.out_channel.send((find_node_message.clone(), *addr)).is_err() {
            warn!("bip_dht: Failed to send outgoing bootstrap message to router...");
        } else {
            handler.current_routers.insert(*addr);
        }
    }
    
    // Send messages to all nodes
    for addr in nodes.iter() {
        if handler.out_channel.send((find_node_message.clone(), *addr)).is_err() {
            warn!("bip_dht: Failed to send outgoing bootstrap message to node...");
        }
    }
}

fn handle_check_bootstrap(handler: &mut DhtHandler, event_loop: &mut EventLoop<DhtHandler>, bucket: usize, timeout: u64) {
    let table_id = handler.routing_table.node_id();
    let target_id = if let Some(id) = flip_id_bit_at_index(table_id, bucket) {
        id
    } else {
        /*if timeout == 1000 {
            handle_schedule_task(event_loop, 2000, IntervalTask::CheckBootstrap(0));
        }*/
        println!("DONE");
        return
    };
    
    let find_node = FindNodeRequest::new(&b"0"[..], table_id, table_id);
    let find_node_message = find_node.encode();
    
    let mut sent_requests = false;
    
    if bucket == 0 || bucket == 1 {
        for node in handler.routing_table.closest_nodes(target_id).take(8) {
            if handler.out_channel.send((find_node_message.clone(), node.addr())).is_err() {
                warn!("bip_dht: Could not send a bootstrap message through out channel...");
            }
            
            sent_requests = true;
        }
    } else {
        let mut buckets = handler.routing_table.buckets().skip(bucket - 2);
        let dummy_bucket = Bucket::new();
        
        // Sloppy probability of the nodes in each bucket to have our target id in their bucket.
        let percent_25_bucket = if let Some(bucket) = buckets.next() {
            match bucket {
                BucketContents::Empty => dummy_bucket.iter(),
                BucketContents::Sorted(b) => b.iter(),
                BucketContents::Assorted(b) => b.iter()
            }
        } else { return };
        let percent_50_bucket = if let Some(bucket) = buckets.next() {
            match bucket {
                BucketContents::Empty => dummy_bucket.iter(),
                BucketContents::Sorted(b) => b.iter(),
                BucketContents::Assorted(b) => b.iter()
            }
        } else { return };
        let percent_100_bucket = if let Some(bucket) = buckets.next() {
            match bucket {
                BucketContents::Empty => dummy_bucket.iter(),
                BucketContents::Sorted(b) => b.iter(),
                BucketContents::Assorted(b) => b.iter()
            }
        } else { return };
        
        // TODO: See why reversing the order here sometimes improves node discovery (maybe since its
        // prefering early buckets which are easier to find it is generating more nodes in early buckets?)
        //let chained_buckets = percent_100_bucket.chain(percent_50_bucket).chain(percent_25_bucket);
        let chained_buckets = percent_25_bucket.chain(percent_50_bucket).chain(percent_100_bucket);
        for node in chained_buckets.filter(|n| n.status() != NodeStatus::Bad && n.status() != NodeStatus::Good).take(8) {
            if handler.out_channel.send((find_node_message.clone(), node.addr())).is_err() {
                warn!("bip_dht: Could not send a bootstrap message through out channel...");
            }
            
            sent_requests = true;
        }
    }
    
    // Print Routing Table
    let mut total = 0;
    for (index, bucket) in handler.routing_table.buckets().enumerate() {
        let num_nodes = match bucket {
            BucketContents::Empty => 0,
            BucketContents::Sorted(b) => b.iter().filter(|n| n.status() == NodeStatus::Good ).count(),
            BucketContents::Assorted(b) => b.iter().filter(|n| n.status() == NodeStatus::Good ).count(),
        };
        total += num_nodes;
                
        if num_nodes != 0 {
            print!("Bucket {}: {} | ", index, num_nodes);
        }
    }
    print!("\nTotal: {}\n\n\n", total);
    
    if !sent_requests {
        handle_check_bootstrap(handler, event_loop, bucket + 1, timeout);
    } else {
        handle_schedule_task(event_loop, timeout, IntervalTask::CheckBootstrap(bucket + 1));
    }
}

//----------------------------------------------------------------------------//

fn handle_schedule_task(event_loop: &mut EventLoop<DhtHandler>, timeout: u64, task: IntervalTask) {
    if event_loop.timeout_ms((timeout, task.clone()), timeout).is_err() {
        error!("bip_dht: Received an error when trying to create a timeout for task {:?}...", task);
    }
}

//----------------------------------------------------------------------------//

fn handle_check_refresh(handler: &mut DhtHandler, event_loop: &mut EventLoop<DhtHandler>, bucket: usize, timeout: u64) {
    
}

//----------------------------------------------------------------------------//

fn handle_start_lookup(handler: &mut DhtHandler, event_loop: &mut EventLoop<DhtHandler>, info_hash: InfoHash) {
    let lookup = HashLookup::new(handler.routing_table.node_id(), info_hash, &handler.routing_table, &handler.out_channel, event_loop);
    
    if !handler.active_lookups.is_empty() {
        match lookup {
            Some(lookup) => handler.active_lookups[0] = lookup,
            None         => ()
        }
    } else {
        match lookup {
            Some(lookup) => handler.active_lookups.push(lookup),
            None         => ()
        }
    }
}

fn handle_check_node_lookup(handler: &mut DhtHandler, event_loop: &mut EventLoop<DhtHandler>, index: usize, node: Node) {
    println!("{:?}", handler.active_lookups[index].node_timeout(node, &handler.out_channel, event_loop));
}

fn handle_check_bulk_lookup(handler: &mut DhtHandler, event_loop: &mut EventLoop<DhtHandler>, index: usize) {
    println!("{:?}", handler.active_lookups[index].bulk_timeout(&handler.out_channel, event_loop));
}

//----------------------------------------------------------------------------//

fn flip_id_bit_at_index(node_id: NodeId, index: usize) -> Option<NodeId> {
    let mut id_bytes: [u8; bip_util::NODE_ID_LEN]  = node_id.into();
    let (byte_index, bit_index) = (index / 8, index % 8);
    
    if byte_index >= bip_util::NODE_ID_LEN {
        None
    } else {
        let actual_bit_index = 7 - bit_index;
        id_bytes[byte_index] ^= 1 << actual_bit_index;
    
        Some(id_bytes.into())
    }
}