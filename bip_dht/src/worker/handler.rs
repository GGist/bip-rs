use std::cell::{Cell};
use std::collections::{HashSet};
use std::io::{self};
use std::iter::{self};
use std::net::{SocketAddr};
use std::sync::{Arc};
use std::sync::mpsc::{SyncSender};
use std::thread::{self};

use bip_bencode::{Bencode};
use bip_util::{self, NodeId};
use mio::{EventLoop, Sender, Timeout, Handler};

use message::{MessageType};
use message::find_node::{FindNodeRequest};
use message::response::{ResponseType, ExpectedResponse};
use router::{Router};
use routing::bucket::{Bucket};
use routing::node::{Node};
use routing::table::{self, RoutingTable};
use worker::{self, OneshotTask, IntervalTask};

use routing::table::{BucketContents};
use routing::node::{NodeStatus};

pub fn create_dht_handler(table: RoutingTable, out: SyncSender<(Vec<u8>, SocketAddr)>)
	-> io::Result<Sender<OneshotTask>> {
	let mut handler = DhtHandler::new(table, out);
	let mut event_loop = try!(EventLoop::new());
	
	let loop_channel = event_loop.channel();
	
	thread::spawn(move || event_loop.run(&mut handler));
	
	Ok(loop_channel)
}

//----------------------------------------------------------------------------//

struct DhtHandler {
	out_channel:     SyncSender<(Vec<u8>, SocketAddr)>,
	routing_table:   RoutingTable,
	current_routers: HashSet<SocketAddr>
}

impl DhtHandler {
	fn new(table: RoutingTable, out: SyncSender<(Vec<u8>, SocketAddr)>) -> DhtHandler {
		DhtHandler{ out_channel: out, routing_table: table, current_routers: HashSet::new() }
	}
}

impl Handler for DhtHandler {
	type Timeout = (u64, IntervalTask);
	type Message = OneshotTask;
	
	fn notify(&mut self, event_loop: &mut EventLoop<DhtHandler>, task: OneshotTask) {
		match task {
			OneshotTask::Incoming(buffer, addr) => {
				handle_incoming(self, &buffer[..], addr);
			},
			OneshotTask::ScheduleTask(timeout, task) => {
				handle_schedule_task(event_loop, timeout, task);
			},
			OneshotTask::StartBootstrap(routers, nodes) => {
				handle_start_bootstrap(self, &routers[..], &nodes[..]);
			}
		}
	}
	
	fn timeout(&mut self, event_loop: &mut EventLoop<DhtHandler>, data: (u64, IntervalTask)) {
		let (timeout, task) = data;
		
		match task {
			IntervalTask::CheckBootstrap(b) => {
				handle_check_bootstrap(self, event_loop, b, timeout);
			},
			IntervalTask::CheckRefresh(b) => {
				handle_check_refresh(self, event_loop, b, timeout);
			}
		}
	}
}

//----------------------------------------------------------------------------//

fn handle_incoming(handler: &mut DhtHandler, buffer: &[u8], addr: SocketAddr) {
	let bencode = if let Ok(b) = Bencode::decode(buffer) {
		b
	} else {
		warn!("bip_dht: Received invalid bencode data...");
		return
	};
	
	match MessageType::new(&bencode, |trans| ExpectedResponse::FindNode) {
		Ok(MessageType::Response(ResponseType::FindNode(f))) => {
			// Add returned nodes as questionable (unpinged in this case)
			for (node_id, v4_addr) in f.nodes() {
				let addr = SocketAddr::V4(v4_addr);
				let node = Node::as_questionable(node_id, addr);
				
				handler.routing_table.add_node(node);
			}
			
			// Add responding node as good
			if !handler.current_routers.contains(&addr) {
				let id = NodeId::from_bytes(f.node_id()).unwrap();
				let node = Node::as_good(id, addr);
				
				handler.routing_table.add_node(node);
			}
		},
		_ => ()
	};
}

fn handle_start_bootstrap(handler: &mut DhtHandler, routers: &[Router], nodes: &[SocketAddr]) {
	let node_id = handler.routing_table.node_id();
	let router_filter = routers.iter().filter_map(|r| r.ipv4_addr().ok().map(|m| SocketAddr::V4(m)) ).collect::<HashSet<SocketAddr>>();
	
	let find_node = FindNodeRequest::new(&b"0"[..], node_id.as_bytes(), node_id.as_bytes()).unwrap();
	let find_node_message = find_node.encode();
	
	// Send messages to all routers
	for addr in router_filter.iter() {
		if handler.out_channel.send((find_node_message.clone(), *addr)).is_err() {
			warn!("bip_dht: Failed to send outgoing bootstrap message to router...");
		} else {
			handler.current_routers.insert(*addr);
		}
	}
}

fn handle_schedule_task(event_loop: &mut EventLoop<DhtHandler>, timeout: u64, task: IntervalTask) {
	if event_loop.timeout_ms((timeout, task), timeout).is_err() {
		error!("bip_dht: Received an error when trying to create a timeout for task {:?}...", task);
	}
}

fn handle_check_bootstrap(handler: &mut DhtHandler, event_loop: &mut EventLoop<DhtHandler>, bucket: usize, timeout: u64) {
	let table_id = handler.routing_table.node_id();
	let target_id = if let Some(id) = flip_id_bit_at_index(table_id, bucket) {
		id
	} else {
		println!("DONE");
		return
	};
	
	let find_node = FindNodeRequest::new(&b"0"[..], table_id.as_bytes(), target_id.as_bytes()).unwrap();
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
		
		let chained_buckets = percent_100_bucket.chain(percent_50_bucket).chain(percent_25_bucket);
		for node in chained_buckets.filter(|n| n.status() != NodeStatus::Bad).take(8) {
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

fn handle_check_refresh(handler: &mut DhtHandler, event_loop: &mut EventLoop<DhtHandler>, bucket: usize, timeout: u64) {
	
}

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