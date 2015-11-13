use std::net::{SocketAddr};
use std::sync::{Arc};
use std::sync::mpsc::{SyncSender};

use bip_bencode::{Bencode};
use bip_util::{NodeId};

use message::{MessageType};
use message::find_node::{FindNodeRequest, FindNodeResponse};
use message::response::{ExpectedResponse, ResponseType};
use router::{Router};
use routing::node::{Node};
use workers::manager::{WorkerStorage, TableBootstrap};

pub fn handle_incoming(storage: Arc<WorkerStorage>, buffer: Vec<u8>, addr: SocketAddr) {
    let bencode_msg = if let Ok(b) = Bencode::decode(&buffer) {
        b
    } else {
        warn!("bip_dht: Worker encountered an invalid bencode message...");
        return
    };
    
    let krpc_msg = MessageType::new(&bencode_msg, |t| {
        if t == &b"0"[..] {
            ExpectedResponse::FindNode
        } else {
            ExpectedResponse::None
        }
    });
    
    match krpc_msg {
        Ok(MessageType::Response(ResponseType::FindNode(find_node_msg))) => {
            handle_incoming_bootstrap(storage, find_node_msg, addr)
        },
        _ => warn!("bip_dht: Received unimplemented message type...")
    };
}

fn handle_incoming_bootstrap<'a>(storage: Arc<WorkerStorage>, find_node_msg: FindNodeResponse<'a>, addr: SocketAddr) {
    match (storage.table_bootstrap.lock(), storage.routing_table.lock()) {
        (Ok(mut boot), Ok(mut table)) => {
            if !boot.discovered_routers.contains(&addr) {
                let node_id = NodeId::from_bytes(find_node_msg.node_id()).unwrap();
                let mut node = Node::new(node_id, addr);
                node.remote_response();
                
                table.add_node(node);
            }
            
            for (_, addr_v4) in find_node_msg.nodes() {
                let socket_addr = SocketAddr::V4(addr_v4);
                
                boot.discovered_nodes.push(socket_addr);
            }
        },
        (Err(_), Ok(_))  => error!("bip_dht: Worker encountered a poisoned mutex on the routing table storage..."),
        (Ok(_), Err(_))  => error!("bip_dht: Worker encountered a poisoned mutex on the bootstrap table storage..."),
        (Err(_), Err(_)) => error!("bip_dht: Worker encountered a poisoned mutex on the routing table storage and bootstrap table storage...")
    }
}

//----------------------------------------------------------------------------//

pub fn handle_start_bootstrap(storage: Arc<WorkerStorage>, routers: Vec<Router>, nodes: Vec<SocketAddr>) {
    match (storage.table_bootstrap.lock(), storage.outgoing_channel.lock()) {
        (Ok(mut table), Ok(out)) => restart_table_bootstrap(&mut table, &out, &routers, &nodes),
        (Err(_), Ok(_))  => error!("bip_dht: Worker encountered a poisoned mutex on the bootstrap table storage..."),
        (Ok(_), Err(_))  => error!("bip_dht: Worker encountered a poisoned mutex on the outgoing channel..."),
        (Err(_), Err(_)) => error!("bip_dht: Worker encountered a poisoned mutex on the bootstrap table storage and outgoing channel...")
    };
}

fn restart_table_bootstrap(bootstrap: &mut TableBootstrap, out: &SyncSender<(Vec<u8>, SocketAddr)>, routers: &[Router],
    nodes: &[SocketAddr]) {
    bootstrap.next_bucket_index = 0;
    bootstrap.active_bootstraps.clear();
    bootstrap.discovered_nodes.clear();
    bootstrap.discovered_routers.clear();
    
    let message = generate_bootstrap_message(bootstrap.table_node_id, bootstrap.table_node_id);
    
    for router in routers {
        if let Ok(addr) = router.socket_addr() {
            if let Err(_) = out.send((message.clone(), addr)) {
                warn!("bip_dht: Channel failed to send an out message to router...");
            } else {
                bootstrap.discovered_routers.insert(addr);
            }
        } else {
            warn!("bip_dht: Could not get a socket address for router on table bootstrap...");
        }
    }
    
    for node in nodes {
        if let Err(_) = out.send((message.clone(), *node)) {
            warn!("bip_dht: Channel failed to send an out message to node...");
        }
    }
}

pub fn handle_check_bootstrap(storage: Arc<WorkerStorage>) {
    let table = match storage.table_bootstrap.lock() {
        Ok(t)  => t,
        Err(_) => {
            warn!("bip_dht: Bootstrap check could not lock the table bootstrap because it is poisoned...");
            return
        }
    };

    // Clean up any bucket bootstraps
    table.active_bootstraps.retain(|b| {
        !b.is_done()
    });
    
    // Push any new bucket bootstraps that we can handle
    let mut next_bucket_index = table.next_bucket_index;
    let mut num_bootstraps = table.active_bootstraps.len();
    let max_bootstraps = table.discovered_nodes.len() / 10;
    
    while num_bootstraps < max_bootstraps && next_bucket_index < 160 {
        
    }
    
    
    
    let (num_bootstraps, num_nodes, next_index) = match storage.table_bootstrap.lock() {
        Ok(table) => {
            table.acvive_bootstraps.retain( |b| {
                !b.is_done()
            });
            
            (table.active_bootstraps.len(), table.discovered_nodes.len(), table.next_bucket_index)
        },
        Err(_) => {
            
            return
        }
    }
    
    // Push any new bucket bootstraps that we can handle
    let max_bootstraps = num_nodes / 10;
    if max_bootstraps > num_bootstraps && next_index < 160 {
        match storage.table_bootstrap.lock() {
            Ok(table) => {
                
            },
            Err(_) => {
                warn!("bip_dht: Bootstrap check could not lock the table bootstrap because it is poisoned...");
                return
            }
        }
    }
    
    // Execute all bucket bootstraps
    
}

fn generate_bootstrap_message(node_id: NodeId, target_id: NodeId) -> Vec<u8> {
    FindNodeRequest::new(&b"0"[..], node_id.as_bytes(), target_id.as_bytes()).unwrap().encode()
}