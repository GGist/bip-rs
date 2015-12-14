use std::collections::{HashMap};
use std::collections::hash_map::{Entry};
use std::convert::{AsRef};
use std::io::{self};
use std::iter::{self};
use std::net::{SocketAddr, UdpSocket};
use std::mem::{self};
use std::sync::mpsc::{self, SyncSender};
use std::thread::{self};

use bip_bencode::{Bencode};
use bip_handshake::{Handshaker};
use bip_util::bt::{InfoHash};
use bip_util::net::{IpAddr};
use mio::{self, EventLoop, Handler};

use message::{MessageType};
use message::ping::{PingResponse};
use message::find_node::{FindNodeResponse};
use message::get_peers::{GetPeersResponse, CompactInfoType};
use message::announce_peer::{AnnouncePeerResponse};
use message::request::{RequestType};
use message::response::{ResponseType, ExpectedResponse};
use message::compact_info::{CompactNodeInfo};
use router::{Router};
use routing::node::{Node};
use routing::table::{RoutingTable};
use token::{TokenStore, Token};
use transaction::{AIDGenerator, TransactionID, ActionID};
use worker::{OneshotTask, ScheduledTask, DhtEvent, ShutdownCause};
use worker::bootstrap::{TableBootstrap, BootstrapStatus};
use worker::lookup::{TableLookup, LookupStatus};
use worker::refresh::{TableRefresh, RefreshStatus};

use routing::table::{BucketContents};
use routing::node::{NodeStatus};

// TODO: Update modules to use find_node on the routing table to update the status of a given node.

const MAX_BOOTSTRAP_ATTEMPTS:         usize = 3;
const BOOTSTRAP_GOOD_NODE_THRESHOLD:  usize = 10;

/// Spawns a DHT handler that maintains our routing table and executes our actions on the DHT.
pub fn create_dht_handler<H>(table: RoutingTable, out: SyncSender<(Vec<u8>, SocketAddr)>, read_only: bool,
    handshaker: H, kill_sock: UdpSocket, kill_addr: SocketAddr) -> io::Result<mio::Sender<OneshotTask>>
    where H: Handshaker + 'static {
    let mut handler = DhtHandler::new(table, out, read_only, handshaker);
    let mut event_loop = try!(EventLoop::new());
    
    let loop_channel = event_loop.channel();
    
    thread::spawn(move || {
        if event_loop.run(&mut handler).is_err() {
            error!("bip_dht: EventLoop shut down with an error...");
        }
        
        // Make sure the handler and event loop are dropped before sending our incoming messenger kill
        // message so that the incoming messenger can not send anything through their event loop channel.
        mem::drop(event_loop);
        mem::drop(handler);
        
        // When event loop stops, we need to "wake" the incoming messenger with a socket message,
        // when it processes the message and tries to pass it to us, it will see that our channel
        // is closed and know that it should shut down. The outgoing messenger will shut itself down.
        // TODO: This will not work if kill_addr is set to a default route 0.0.0.0, need to find another
        // work around (potentially findind out the actual addresses for the current machine beforehand?)
        if kill_sock.send_to(&b"0"[..], kill_addr).is_err() {
            error!("bip_dht: Failed to send a wake up message to the incoming channel...");
        }
        
        info!("bip_dht: DhtHandler gracefully shut down, exiting thread...");
    });
    
    Ok(loop_channel)
}

//----------------------------------------------------------------------------//

pub struct DhtHandler<H> {
    read_only:       bool,
    refresher:       (ActionID, TableRefresh),
    handshaker:      H,
    out_channel:     SyncSender<(Vec<u8>, SocketAddr)>,
    token_store:     TokenStore,
    // Storing the number of bootstrap attempts.
    bootstrapper:    (ActionID, TableBootstrap, usize),
    aid_generator:   AIDGenerator,
    routing_table:   RoutingTable,
    active_stores:   HashMap<InfoHash, Vec<SocketAddr>>,
    active_lookups:  HashMap<ActionID, TableLookup>,
    event_notifiers: Vec<mpsc::Sender<DhtEvent>>
}

impl<H> DhtHandler<H> where H: Handshaker {
    fn new(table: RoutingTable, out: SyncSender<(Vec<u8>, SocketAddr)>, read_only: bool, handshaker: H)
        -> DhtHandler<H> {
        let mut aid_generator = AIDGenerator::new();
        
        let boot_mid_generator = aid_generator.generate();
        let boot_action_id = boot_mid_generator.action_id();
        let table_bootstrap = TableBootstrap::new(table.node_id(), boot_mid_generator, Vec::new(), iter::empty());
    
        let refresh_mid_generator = aid_generator.generate();
        let refresh_action_id = refresh_mid_generator.action_id();
        let table_refresh = TableRefresh::new(refresh_mid_generator);
    
        DhtHandler{ read_only: read_only, refresher: (refresh_action_id, table_refresh), handshaker: handshaker,
            out_channel: out, bootstrapper: (boot_action_id, table_bootstrap, 0), token_store: TokenStore::new(),
            aid_generator: aid_generator, routing_table: table, active_stores: HashMap::new(), active_lookups: HashMap::new(),
            event_notifiers: Vec::new() }
    }
}

impl<H> Handler for DhtHandler<H> where H: Handshaker {
    type Timeout = (u64, ScheduledTask);
    type Message = OneshotTask;
    
    fn notify(&mut self, event_loop: &mut EventLoop<DhtHandler<H>>, task: OneshotTask) {
        match task {
            OneshotTask::Incoming(buffer, addr) => {
                handle_incoming(self, event_loop, &buffer[..], addr);
            },
            OneshotTask::RegisterSender(send) => {
                handle_register_sender(self, send);
            }
            OneshotTask::ScheduleTask(timeout, task) => {
                handle_schedule_task(event_loop, timeout, task);
            },
            OneshotTask::StartBootstrap(routers, nodes) => {
                handle_start_bootstrap(self, event_loop, routers, nodes);
            },
            OneshotTask::StartLookup(info_hash, should_announce) => {
                handle_start_lookup(self, event_loop, info_hash, should_announce);
            },
            OneshotTask::Shutdown(cause) => {
                handle_shutdown(self, event_loop, cause);
            }
        }
    }
    
    fn timeout(&mut self, event_loop: &mut EventLoop<DhtHandler<H>>, data: (u64, ScheduledTask)) {
        let (_, task) = data;
        
        match task {
            ScheduledTask::CheckTableRefresh(trans_id) => {
                handle_check_table_refresh(self, event_loop, trans_id);
            },
            ScheduledTask::CheckBootstrapTimeout(trans_id) => {
                handle_check_bootstrap_timeout(self, event_loop, trans_id);
            },
            ScheduledTask::CheckLookupTimeout(trans_id) => {
                handle_check_lookup_timeout(self, event_loop, trans_id);
            },
            ScheduledTask::CheckLookupEndGame(trans_id) => {
                handle_check_lookup_endgame(self, event_loop, trans_id);
            }
        }
    }
}

//----------------------------------------------------------------------------//

/// Shut down the event loop by sending it a shutdown message with the given cause.
fn shutdown_event_loop<H>(event_loop: &mut EventLoop<DhtHandler<H>>, cause: ShutdownCause)
    where H: Handshaker {
    if event_loop.channel().send(OneshotTask::Shutdown(cause)).is_err() {
        error!("bip_dht: Failed to sent a shutdown message to the EventLoop...");
    }
}

/// Broadcast the given event to all of the event nodifiers.
fn broadcast_dht_event(notifiers: &mut Vec<mpsc::Sender<DhtEvent>>, event: DhtEvent) {
    notifiers.retain(|send| send.send(event).is_ok() );
}

/// Number of good nodes in the RoutingTable.
fn num_good_nodes(table: &RoutingTable) -> usize {
    table.closest_nodes(table.node_id()).filter(|n| n.status() == NodeStatus::Good).count()
}

/// We should rebootstrap if we have a low number of nodes.
fn should_rebootstrap(table: &RoutingTable) -> bool {
    num_good_nodes(table) <= BOOTSTRAP_GOOD_NODE_THRESHOLD
}

/// Broadcast that the bootstrap has completed.
/// IMPORTANT: Should call this instead of broadcast_dht_event()!
fn broadcast_bootstrap_completed<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>)
    where H: Handshaker {
    // Send notification that the bootstrap has completed.
    broadcast_dht_event(&mut handler.event_notifiers, DhtEvent::BootstrapCompleted);
    
    // Start the refresh process.
    let refresh_status = handler.refresher.1.continue_refresh(&handler.routing_table, &handler.out_channel, event_loop);
    match refresh_status {
        RefreshStatus::Refreshing => (),
        RefreshStatus::Failed     => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
    };
}

/// Attempt to rebootstrap or shutdown the dht if we have no nodes after rebootstrapping multiple time.
fn attempt_rebootstrap<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>)
    where H: Handshaker {
    // Increment bootstrap counter
    handler.bootstrapper.2 += 1;
    
    // Check if we reached the maximum bootstrap attempts
    if handler.bootstrapper.2 >= MAX_BOOTSTRAP_ATTEMPTS {
        // Should not shutdown if we have a non zero number of nodes
        if num_good_nodes(&handler.routing_table) != 0 {
            // Make do with what we have, but dont rebootstrap again
            return
        } else {
            // Failed to get any nodes in the rebootstrap attempts, shut down
            shutdown_event_loop(event_loop, ShutdownCause::BootstrapFailed);
        }
    }
    
    warn!("bip_dht: Bootstrap attempt {} failed, attempting a rebootstrap...", handler.bootstrapper.2);
    let bootstrap_status = handler.bootstrapper.1.start_bootstrap(&handler.out_channel, event_loop);
    match bootstrap_status {
        BootstrapStatus::Idle          => (),
        BootstrapStatus::Bootstrapping => (),
        BootstrapStatus::Completed     => {
            // Check if our bootstrap was actually good
            if should_rebootstrap(&handler.routing_table) {
                attempt_rebootstrap(handler, event_loop);
            } else {
                broadcast_bootstrap_completed(handler, event_loop);
            }
        },
        BootstrapStatus::Failed => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
    };
}

//----------------------------------------------------------------------------//

fn handle_incoming<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, buffer: &[u8], addr: SocketAddr)
    where H: Handshaker {
    // Parse the buffer as a bencoded message
    let bencode = if let Ok(b) = Bencode::decode(buffer) {
        b
    } else {
        warn!("bip_dht: Received invalid bencode data...");
        return
    };
    
    // Parse the bencode as a message
    // Check to make sure we issued the transaction id (or that it is still valid)
    let message = MessageType::new(&bencode, |trans| {
        let trans_id = if let Some(t) = TransactionID::from_bytes(trans) {
            t 
        } else {
            return ExpectedResponse::None
        };
        
        if trans_id.action_id() == handler.bootstrapper.0 || trans_id.action_id() == handler.refresher.0 {
            ExpectedResponse::FindNode
        } else if handler.active_lookups.contains_key(&trans_id.action_id()) {
            ExpectedResponse::GetPeers
        } else {
            ExpectedResponse::None
        }
    });
    
    // Do not process requests if we are read only
    if handler.read_only {
        match message {
            Ok(MessageType::Request(_)) => return,
            _                           => ()
        }
    }
    
    // Process the given message
    match message {
        Ok(MessageType::Request(RequestType::Ping(p))) => {
            info!("bip_dht: Received a PingRequest...");
            
            let ping_rsp = PingResponse::new(p.transaction_id(), handler.routing_table.node_id());
            let ping_msg = ping_rsp.encode();
            
            if handler.out_channel.send((ping_msg, addr)).is_err() {
                error!("bip_dht: Failed to send a ping response on the out channel...");
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
            }
        },
        Ok(MessageType::Request(RequestType::FindNode(f))) => {
            info!("bip_dht: Received a FindNodeRequest...");
            
            // Grab the closest nodes
            let mut closest_nodes_bytes = Vec::with_capacity(26 * 8);
            for node in handler.routing_table.closest_nodes(f.target_id()).take(8) {
                closest_nodes_bytes.extend_from_slice(&node.encode());
            }
            
            let find_node_rsp = FindNodeResponse::new(f.transaction_id(), handler.routing_table.node_id(),
                &closest_nodes_bytes).unwrap();
            let find_node_msg = find_node_rsp.encode();
            
            if handler.out_channel.send((find_node_msg, addr)).is_err() {
                error!("bip_dht: Failed to send a find node response on the out channel...");
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
            }
        },
        Ok(MessageType::Request(RequestType::GetPeers(g))) => {
            info!("bip_dht: Received a GetPeersRequest...");
            // TODO: Check if we have values for the given info hash
            
            // Otherwise return closest nodes
            let mut closest_nodes_bytes = Vec::with_capacity(26 * 8);
            for node in handler.routing_table.closest_nodes(g.info_hash()).take(8) {
                closest_nodes_bytes.extend_from_slice(&node.encode());
            }
            
            let token = handler.token_store.checkout(IpAddr::from_socket_addr(addr));
            let get_peers_rsp = GetPeersResponse::new(g.transaction_id(), handler.routing_table.node_id(),
                Some(token.as_ref()), CompactInfoType::Nodes(CompactNodeInfo::new(&closest_nodes_bytes).unwrap()));
            let get_peers_msg = get_peers_rsp.encode();
            
            if handler.out_channel.send((get_peers_msg, addr)).is_err() {
                error!("bip_dht: Failed to send a get peers response on the out channel...");
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
            }
        },
        Ok(MessageType::Request(RequestType::AnnouncePeer(a))) => {
            info!("bip_dht: Received an AnnouncePeerRequest...");
            
            // Validate the token
            let is_valid = match Token::new(a.token()) {
                Ok(t) => {
                    handler.token_store.checkin(IpAddr::from_socket_addr(addr), t)
                },
                Err(_) => false
            };
            
            if !is_valid {
                warn!("bip_dht: Node sent us an invalid announce token...");
                return
            }
            
            // Store the value
            // TODO: Add a cap and an expire time for stored values
            match handler.active_stores.entry(a.info_hash()) {
                Entry::Occupied(mut occ) => {
                    occ.get_mut().push(addr);
                },
                Entry::Vacant(vac) => {
                    vac.insert(vec![addr]);
                }
            }
            
            let announce_peer_rsp = AnnouncePeerResponse::new(a.transaction_id(), handler.routing_table.node_id());
            let announce_peer_msg = announce_peer_rsp.encode();
            
            if handler.out_channel.send((announce_peer_msg, addr)).is_err() {
                error!("bip_dht: Failed to send an announce peer response on the out channel...");
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
            }
        },
        Ok(MessageType::Response(ResponseType::FindNode(f))) => {
            info!("bip_dht: Received a FindNodeResponse...");
            let trans_id = TransactionID::from_bytes(f.transaction_id()).unwrap();
            let node = Node::as_good(f.node_id(), addr);
            
            // Add the responding node if it is not a router
            if !handler.bootstrapper.1.is_router(&node.addr()) {
                handler.routing_table.add_node(node);
            }
            
            // Add the other nodes as questionable
            for (id, v4_addr) in f.nodes() {
                let sock_addr = SocketAddr::V4(v4_addr);
                
                handler.routing_table.add_node(Node::as_questionable(id, sock_addr));
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
            
            
            // If this was a refresh response, dont do anything else
            if trans_id.action_id() == handler.refresher.0 {
                return
            }
            // Was a bootstrap response
            
            // Let the bootstrapper know of the message response
            let bootstrap_status = handler.bootstrapper.1.recv_response(&trans_id, &handler.routing_table, &handler.out_channel, event_loop);
            match bootstrap_status {
                BootstrapStatus::Idle          => (),
                BootstrapStatus::Bootstrapping => (),
                BootstrapStatus::Completed     => {
                    // Check if our bootstrap was actually good
                    if should_rebootstrap(&handler.routing_table) {
                        attempt_rebootstrap(handler, event_loop);
                    } else {
                        broadcast_bootstrap_completed(handler, event_loop);
                    }
                },
                BootstrapStatus::Failed => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
            };
        },
        Ok(MessageType::Response(ResponseType::GetPeers(g))) => {
           info!("bip_dht: Received a GetPeersResponse...");
            let trans_id = TransactionID::from_bytes(g.transaction_id()).unwrap();
            let node = Node::as_good(g.node_id(), addr);
            
            // Add the responding node if it is not a router
            if !handler.bootstrapper.1.is_router(&node.addr()) {
                handler.routing_table.add_node(node.clone());
            }
            
            if let Some(mut lookup) = handler.active_lookups.get_mut(&trans_id.action_id()) {
                match lookup.recv_response(node, &trans_id, g, &handler.out_channel, event_loop) {
                    LookupStatus::Searching => (),
                    LookupStatus::Values(v) => {
                        // Add values to handshaker
                        for v4_addr in v {
                            let sock_addr = SocketAddr::V4(v4_addr);
                            
                            handler.handshaker.connect(None, lookup.info_hash(), sock_addr);
                        }
                    },
                    LookupStatus::Completed => broadcast_dht_event(&mut handler.event_notifiers, DhtEvent::LookupCompleted(lookup.info_hash())),
                    LookupStatus::Failed    => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
                }
            }
        },
        Ok(MessageType::Response(ResponseType::Ping(_))) => {
            info!("bip_dht: Received a PingResponse...");
            
            // Yeah...we should never be getting this type of response (we never use this message)
        },
        Ok(MessageType::Response(ResponseType::AnnouncePeer(_))) => {
            info!("bip_dht: Received an AnnouncePeerResponse...");
        },
        Ok(MessageType::Error(e)) => {
            info!("bip_dht: Received an ErrorMessage...");
            
            warn!("bip_dht: KRPC error message from {:?}: {:?}", addr, e);
        },
        Err(e) => {
            warn!("bip_dht: Error parsing KRPC message: {:?}", e);
        }
    }
}

fn handle_register_sender<H>(handler: &mut DhtHandler<H>, sender: mpsc::Sender<DhtEvent>) {
    handler.event_notifiers.push(sender);
}

fn handle_schedule_task<H>(event_loop: &mut EventLoop<DhtHandler<H>>, timeout: u64, task: ScheduledTask)
    where H: Handshaker {
    if event_loop.timeout_ms((timeout, task), timeout).is_err() {
        shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
    }
}

fn handle_start_bootstrap<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, routers: Vec<Router>, nodes: Vec<SocketAddr>)
    where H: Handshaker {
    let router_iter = routers.into_iter().filter_map(|r| r.ipv4_addr().ok().map(|v4| SocketAddr::V4(v4)) );
    
    let mid_generator = handler.aid_generator.generate();
    let action_id = mid_generator.action_id();
    
    let table_bootstrap = TableBootstrap::new(handler.routing_table.node_id(), mid_generator, nodes, router_iter);
    
    handler.bootstrapper = (action_id, table_bootstrap, 0);
    
    // Begin the bootstrap operation
    let bootstrap_status = handler.bootstrapper.1.start_bootstrap(&handler.out_channel, event_loop);
    
    match bootstrap_status {
        BootstrapStatus::Idle          => (),
        BootstrapStatus::Bootstrapping => (),
        BootstrapStatus::Completed     => {
            // Check if our bootstrap was actually good
            if should_rebootstrap(&handler.routing_table) {
                attempt_rebootstrap(handler, event_loop);
            } else {
                broadcast_bootstrap_completed(handler, event_loop);
            }
        },
        BootstrapStatus::Failed => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
    };
}

fn handle_start_lookup<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, info_hash: InfoHash, should_announce: bool)
    where H: Handshaker {
    let mid_generator = handler.aid_generator.generate();
    let action_id = mid_generator.action_id();
    
    match TableLookup::new(handler.routing_table.node_id(), info_hash, mid_generator, &handler.routing_table, &handler.out_channel, event_loop) {
        Some(lookup) => { handler.active_lookups.insert(action_id, lookup); },
        None         => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
    }
}

fn handle_shutdown<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, cause: ShutdownCause)
    where H: Handshaker {
    broadcast_dht_event(&mut handler.event_notifiers, DhtEvent::ShuttingDown(cause));
    
    event_loop.shutdown();
}

fn handle_check_table_refresh<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, _: TransactionID)
    where H: Handshaker {
    let refresh_status = handler.refresher.1.continue_refresh(&handler.routing_table, &handler.out_channel, event_loop);
    
    match refresh_status {
        RefreshStatus::Refreshing => (),
        RefreshStatus::Failed     => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
    };
}

fn handle_check_bootstrap_timeout<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, trans_id: TransactionID)
    where H: Handshaker {
    let bootstrap_status = handler.bootstrapper.1.recv_timeout(&trans_id, &handler.routing_table, &handler.out_channel, event_loop);
    
    match bootstrap_status {
        BootstrapStatus::Idle          => (),
        BootstrapStatus::Bootstrapping => (),
        BootstrapStatus::Completed     => {
            // Check if our bootstrap was actually good
            if should_rebootstrap(&handler.routing_table) {
                attempt_rebootstrap(handler, event_loop);
            } else {
                broadcast_bootstrap_completed(handler, event_loop);
            }
        },
        BootstrapStatus::Failed => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
    };
}

fn handle_check_lookup_timeout<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, trans_id: TransactionID)
    where H: Handshaker {
    if let Some(mut lookup) = handler.active_lookups.get_mut(&trans_id.action_id()) {
        match lookup.recv_timeout(&trans_id, &handler.out_channel, event_loop) {
            LookupStatus::Searching => (),
            LookupStatus::Values(v) => {
                // Add values to handshaker
                for v4_addr in v {
                    let sock_addr = SocketAddr::V4(v4_addr);
                
                    handler.handshaker.connect(None, lookup.info_hash(), sock_addr);
                }
            },
            LookupStatus::Completed => broadcast_dht_event(&mut handler.event_notifiers, DhtEvent::LookupCompleted(lookup.info_hash())),
            LookupStatus::Failed    => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
        }
    }
}

fn handle_check_lookup_endgame<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, trans_id: TransactionID)
    where H: Handshaker {
    if let Some(mut lookup) = handler.active_lookups.remove(&trans_id.action_id()) {
        match lookup.recv_finished(&handler.out_channel) {
            LookupStatus::Searching => (),
            LookupStatus::Values(v) => {
                // Add values to handshaker
                for v4_addr in v {
                    let sock_addr = SocketAddr::V4(v4_addr);
                
                    handler.handshaker.connect(None, lookup.info_hash(), sock_addr);
                }
            },
            LookupStatus::Completed => broadcast_dht_event(&mut handler.event_notifiers, DhtEvent::LookupCompleted(lookup.info_hash())),
            LookupStatus::Failed    => shutdown_event_loop(event_loop, ShutdownCause::Unspecified)
        }
    }
}