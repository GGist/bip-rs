use std::collections::HashMap;
use std::convert::AsRef;
use std::io;
use std::mem;
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket};
use std::sync::mpsc::{self, SyncSender};
use std::thread;

use bip_bencode::Bencode;
use bip_handshake::Handshaker;
use bip_util::bt::InfoHash;
use bip_util::convert;
use bip_util::net::IpAddr;
use log::LogLevel;
use mio::{self, EventLoop, Handler};

use crate::message::announce_peer::{AnnouncePeerResponse, ConnectPort};
use crate::message::compact_info::{CompactNodeInfo, CompactValueInfo};
use crate::message::error::{ErrorCode, ErrorMessage};
use crate::message::find_node::FindNodeResponse;
use crate::message::get_peers::{CompactInfoType, GetPeersResponse};
use crate::message::ping::PingResponse;
use crate::message::request::RequestType;
use crate::message::response::{ExpectedResponse, ResponseType};
use crate::message::MessageType;
use crate::router::Router;
use crate::routing::node::Node;
use crate::routing::table::RoutingTable;
use crate::storage::AnnounceStorage;
use crate::token::{Token, TokenStore};
use crate::transaction::{AIDGenerator, ActionID, TransactionID};
use crate::worker::bootstrap::{BootstrapStatus, TableBootstrap};
use crate::worker::lookup::{LookupStatus, TableLookup};
use crate::worker::refresh::{RefreshStatus, TableRefresh};
use crate::worker::{DhtEvent, OneshotTask, ScheduledTask, ShutdownCause};

use crate::routing::node::NodeStatus;
use crate::routing::table::BucketContents;

// TODO: Update modules to use find_node on the routing table to update the status of a given node.

const MAX_BOOTSTRAP_ATTEMPTS: usize = 3;
const BOOTSTRAP_GOOD_NODE_THRESHOLD: usize = 10;

/// Spawns a DHT handler that maintains our routing table and executes our actions on the DHT.
pub fn create_dht_handler<H>(
    table: RoutingTable,
    out: SyncSender<(Vec<u8>, SocketAddr)>,
    read_only: bool,
    handshaker: H,
    kill_sock: UdpSocket,
    kill_addr: SocketAddr,
) -> io::Result<mio::Sender<OneshotTask>>
where
    H: Handshaker + 'static,
{
    let mut handler = DhtHandler::new(table, out, read_only, handshaker);
    let mut event_loop = EventLoop::new()?;

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
        // work around (potentially finding out the actual addresses for the current machine beforehand?)
        if kill_sock.send_to(&b"0"[..], kill_addr).is_err() {
            error!("bip_dht: Failed to send a wake up message to the incoming channel...");
        }

        info!("bip_dht: DhtHandler gracefully shut down, exiting thread...");
    });

    Ok(loop_channel)
}

// ----------------------------------------------------------------------------//

/// Actions that we can perform on our RoutingTable.
enum TableAction {
    /// Lookup action.
    Lookup(TableLookup),
    /// Refresh action.
    Refresh(TableRefresh),
    /// Bootstrap action.
    ///
    /// Includes number of bootstrap attempts.
    Bootstrap(TableBootstrap, usize),
}

/// Actions that we want to perform on our RoutingTable after bootstrapping finishes.
enum PostBootstrapAction {
    /// Future lookup action.
    Lookup(InfoHash, bool),
    /// Future refresh action.
    Refresh(TableRefresh, TransactionID),
}

/// Storage for our EventLoop to invoke actions upon.
pub struct DhtHandler<H> {
    detached: DetachedDhtHandler<H>,
    table_actions: HashMap<ActionID, TableAction>,
}

/// Storage separate from the table actions allowing us to hold mutable references
/// to table actions while still being able to pass around the bulky parameters.
struct DetachedDhtHandler<H> {
    read_only: bool,
    handshaker: H,
    out_channel: SyncSender<(Vec<u8>, SocketAddr)>,
    token_store: TokenStore,
    aid_generator: AIDGenerator,
    bootstrapping: bool,
    routing_table: RoutingTable,
    active_stores: AnnounceStorage,
    // If future actions is not empty, that means we are still bootstrapping
    // since we will always spin up a table refresh action after bootstrapping.
    future_actions: Vec<PostBootstrapAction>,
    event_notifiers: Vec<mpsc::Sender<DhtEvent>>,
}

impl<H> DhtHandler<H>
where
    H: Handshaker,
{
    fn new(table: RoutingTable, out: SyncSender<(Vec<u8>, SocketAddr)>, read_only: bool, handshaker: H) -> DhtHandler<H> {
        let mut aid_generator = AIDGenerator::new();

        // Insert the refresh task to execute after the bootstrap
        let mut mid_generator = aid_generator.generate();
        let refresh_trans_id = mid_generator.generate();
        let table_refresh = TableRefresh::new(mid_generator);
        let future_actions = vec![PostBootstrapAction::Refresh(table_refresh, refresh_trans_id)];

        let detached = DetachedDhtHandler {
            read_only,
            handshaker,
            out_channel: out,
            token_store: TokenStore::new(),
            aid_generator,
            bootstrapping: false,
            routing_table: table,
            active_stores: AnnounceStorage::new(),
            future_actions,
            event_notifiers: Vec::new(),
        };

        DhtHandler {
            detached,
            table_actions: HashMap::new(),
        }
    }
}

impl<H> Handler for DhtHandler<H>
where
    H: Handshaker,
{
    type Timeout = (u64, ScheduledTask);
    type Message = OneshotTask;

    fn notify(&mut self, event_loop: &mut EventLoop<DhtHandler<H>>, task: OneshotTask) {
        match task {
            OneshotTask::Incoming(buffer, addr) => {
                handle_incoming(self, event_loop, &buffer[..], addr);
            },
            OneshotTask::RegisterSender(send) => {
                handle_register_sender(self, send);
            },
            OneshotTask::StartBootstrap(routers, nodes) => {
                handle_start_bootstrap(self, event_loop, routers, nodes);
            },
            OneshotTask::StartLookup(info_hash, should_announce) => {
                handle_start_lookup(&mut self.table_actions, &mut self.detached, event_loop, info_hash, should_announce);
            },
            OneshotTask::Shutdown(cause) => {
                handle_shutdown(self, event_loop, cause);
            },
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<DhtHandler<H>>, data: (u64, ScheduledTask)) {
        let (_, task) = data;

        match task {
            ScheduledTask::CheckTableRefresh(trans_id) => {
                handle_check_table_refresh(&mut self.table_actions, &mut self.detached, event_loop, trans_id);
            },
            ScheduledTask::CheckBootstrapTimeout(trans_id) => {
                handle_check_bootstrap_timeout(self, event_loop, trans_id);
            },
            ScheduledTask::CheckLookupTimeout(trans_id) => {
                handle_check_lookup_timeout(self, event_loop, trans_id);
            },
            ScheduledTask::CheckLookupEndGame(trans_id) => {
                handle_check_lookup_endgame(self, event_loop, trans_id);
            },
        }
    }
}

// ----------------------------------------------------------------------------//

/// Shut down the event loop by sending it a shutdown message with the given cause.
fn shutdown_event_loop<H>(event_loop: &mut EventLoop<DhtHandler<H>>, cause: ShutdownCause)
where
    H: Handshaker,
{
    if event_loop.channel().send(OneshotTask::Shutdown(cause)).is_err() {
        error!("bip_dht: Failed to sent a shutdown message to the EventLoop...");
    }
}

/// Broadcast the given event to all of the event nodifiers.
fn broadcast_dht_event(notifiers: &mut Vec<mpsc::Sender<DhtEvent>>, event: DhtEvent) {
    notifiers.retain(|send| send.send(event).is_ok());
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
fn broadcast_bootstrap_completed<H>(
    action_id: ActionID,
    table_actions: &mut HashMap<ActionID, TableAction>,
    work_storage: &mut DetachedDhtHandler<H>,
    event_loop: &mut EventLoop<DhtHandler<H>>,
) where
    H: Handshaker,
{
    // Send notification that the bootstrap has completed.
    broadcast_dht_event(&mut work_storage.event_notifiers, DhtEvent::BootstrapCompleted);

    // Indicates we are out of the bootstrapping phase
    work_storage.bootstrapping = false;

    // Remove the bootstrap action from our table actions
    table_actions.remove(&action_id);

    // Start the post bootstrap actions.
    let mut future_actions = work_storage.future_actions.split_off(0);
    for table_action in future_actions.drain(..) {
        match table_action {
            PostBootstrapAction::Lookup(info_hash, should_announce) => {
                handle_start_lookup(table_actions, work_storage, event_loop, info_hash, should_announce);
            },
            PostBootstrapAction::Refresh(refresh, trans_id) => {
                table_actions.insert(trans_id.action_id(), TableAction::Refresh(refresh));

                handle_check_table_refresh(table_actions, work_storage, event_loop, trans_id);
            },
        }
    }
}

/// Attempt to rebootstrap or shutdown the dht if we have no nodes after rebootstrapping multiple time.
/// Returns None if the DHT is shutting down, Some(true) if the rebootstrap process started, Some(false) if a rebootstrap is not necessary.
fn attempt_rebootstrap<H>(
    bootstrap: &mut TableBootstrap,
    attempts: &mut usize,
    work_storage: &mut DetachedDhtHandler<H>,
    event_loop: &mut EventLoop<DhtHandler<H>>,
) -> Option<bool>
where
    H: Handshaker,
{
    // Increment the bootstrap counter
    *attempts += 1;

    warn!("bip_dht: Bootstrap attempt {} failed, attempting a rebootstrap...", *attempts);

    // Check if we reached the maximum bootstrap attempts
    if *attempts >= MAX_BOOTSTRAP_ATTEMPTS {
        if num_good_nodes(&work_storage.routing_table) == 0 {
            // Failed to get any nodes in the rebootstrap attempts, shut down
            shutdown_event_loop(event_loop, ShutdownCause::BootstrapFailed);
            None
        } else {
            Some(false)
        }
    } else {
        let bootstrap_status = bootstrap.start_bootstrap(&work_storage.out_channel, event_loop);

        match bootstrap_status {
            BootstrapStatus::Idle => Some(false),
            BootstrapStatus::Bootstrapping => Some(true),
            BootstrapStatus::Failed => {
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
                None
            },
            BootstrapStatus::Completed => {
                if should_rebootstrap(&work_storage.routing_table) {
                    attempt_rebootstrap(bootstrap, attempts, work_storage, event_loop)
                } else {
                    Some(false)
                }
            },
        }
    }
}

// ----------------------------------------------------------------------------//

fn handle_incoming<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, buffer: &[u8], addr: SocketAddr)
where
    H: Handshaker,
{
    let (work_storage, table_actions) = (&mut handler.detached, &mut handler.table_actions);

    // Parse the buffer as a bencoded message
    let bencode = if let Ok(b) = Bencode::decode(buffer) {
        b
    } else {
        warn!("bip_dht: Received invalid bencode data...");
        return;
    };

    // Parse the bencode as a message
    // Check to make sure we issued the transaction id (or that it is still valid)
    let message = MessageType::new(&bencode, |trans| {
        // Check if we can interpret the response transaction id as one of ours.
        let trans_id = if let Some(t) = TransactionID::from_bytes(trans) {
            t
        } else {
            return ExpectedResponse::None;
        };

        // Match the response action id with our current actions
        match table_actions.get(&trans_id.action_id()) {
            Some(&TableAction::Lookup(_)) => ExpectedResponse::GetPeers,
            Some(&TableAction::Refresh(_)) => ExpectedResponse::FindNode,
            Some(&TableAction::Bootstrap(_, _)) => ExpectedResponse::FindNode,
            None => ExpectedResponse::None,
        }
    });

    // Do not process requests if we are read only
    // TODO: Add read only flags to messages we send it we are read only!
    // Also, check for read only flags on responses we get before adding nodes
    // to our RoutingTable.
    if work_storage.read_only {
        match message {
            Ok(MessageType::Request(_)) => return,
            _ => (),
        }
    }

    // Process the given message
    match message {
        Ok(MessageType::Request(RequestType::Ping(p))) => {
            info!("bip_dht: Received a PingRequest...");
            let node = Node::as_good(p.node_id(), addr);

            // Node requested from us, mark it in the Routingtable
            if let Some(n) = work_storage.routing_table.find_node(&node) {
                n.remote_request()
            }

            let ping_rsp = PingResponse::new(p.transaction_id(), work_storage.routing_table.node_id());
            let ping_msg = ping_rsp.encode();

            if work_storage.out_channel.send((ping_msg, addr)).is_err() {
                error!("bip_dht: Failed to send a ping response on the out channel...");
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
            }
        },
        Ok(MessageType::Request(RequestType::FindNode(f))) => {
            info!("bip_dht: Received a FindNodeRequest...");
            let node = Node::as_good(f.node_id(), addr);

            // Node requested from us, mark it in the Routingtable
            if let Some(n) = work_storage.routing_table.find_node(&node) {
                n.remote_request()
            }

            // Grab the closest nodes
            let mut closest_nodes_bytes = Vec::with_capacity(26 * 8);
            for node in work_storage.routing_table.closest_nodes(f.target_id()).take(8) {
                closest_nodes_bytes.extend_from_slice(&node.encode());
            }

            let find_node_rsp = FindNodeResponse::new(f.transaction_id(), work_storage.routing_table.node_id(), &closest_nodes_bytes).unwrap();
            let find_node_msg = find_node_rsp.encode();

            if work_storage.out_channel.send((find_node_msg, addr)).is_err() {
                error!("bip_dht: Failed to send a find node response on the out channel...");
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
            }
        },
        Ok(MessageType::Request(RequestType::GetPeers(g))) => {
            info!("bip_dht: Received a GetPeersRequest...");
            let node = Node::as_good(g.node_id(), addr);

            // Node requested from us, mark it in the Routingtable
            if let Some(n) = work_storage.routing_table.find_node(&node) {
                n.remote_request()
            }

            // TODO: Move socket address serialization code into bip_util
            // TODO: Check what the maximum number of values we can give without overflowing a udp packet
            // Also, if we arent going to give all of the contacts, we may want to shuffle which ones we give
            let mut contact_info_bytes = Vec::with_capacity(6 * 20);
            work_storage.active_stores.find_items(&g.info_hash(), |addr| {
                let mut bytes = [0u8; 6];
                let port = addr.port();

                match addr {
                    SocketAddr::V4(v4_addr) => {
                        for (src, dst) in convert::ipv4_to_bytes_be(*v4_addr.ip()).iter().zip(bytes.iter_mut()) {
                            *dst = *src;
                        }
                    },
                    SocketAddr::V6(_) => {
                        error!("AnnounceStorage contained an IPv6 Address...");
                        return;
                    },
                };

                bytes[4] = (port >> 8) as u8;
                bytes[5] = (port & 0x00FF) as u8;

                contact_info_bytes.extend_from_slice(&bytes);
            });
            // Grab the bencoded list (ugh, we really have to do this, better apis I say!!!)
            let mut contact_info_bencode = Vec::with_capacity(contact_info_bytes.len() / 6);
            for chunk_index in 0..(contact_info_bytes.len() / 6) {
                let (start, end) = (chunk_index * 6, chunk_index * 6 + 6);

                contact_info_bencode.push(ben_bytes!(&contact_info_bytes[start..end]));
            }

            // Grab the closest nodes
            let mut closest_nodes_bytes = Vec::with_capacity(26 * 8);
            for node in work_storage.routing_table.closest_nodes(g.info_hash()).take(8) {
                closest_nodes_bytes.extend_from_slice(&node.encode());
            }

            // Wrap up the nodes/values we are going to be giving them
            let token = work_storage.token_store.checkout(IpAddr::from_socket_addr(addr));
            let comapct_info_type = if !contact_info_bencode.is_empty() {
                CompactInfoType::Both(
                    CompactNodeInfo::new(&closest_nodes_bytes).unwrap(),
                    CompactValueInfo::new(&contact_info_bencode).unwrap(),
                )
            } else {
                CompactInfoType::Nodes(CompactNodeInfo::new(&closest_nodes_bytes).unwrap())
            };

            let get_peers_rsp = GetPeersResponse::new(
                g.transaction_id(),
                work_storage.routing_table.node_id(),
                Some(token.as_ref()),
                comapct_info_type,
            );
            let get_peers_msg = get_peers_rsp.encode();

            if work_storage.out_channel.send((get_peers_msg, addr)).is_err() {
                error!("bip_dht: Failed to send a get peers response on the out channel...");
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
            }
        },
        Ok(MessageType::Request(RequestType::AnnouncePeer(a))) => {
            info!("bip_dht: Received an AnnouncePeerRequest...");
            let node = Node::as_good(a.node_id(), addr);

            // Node requested from us, mark it in the Routingtable
            if let Some(n) = work_storage.routing_table.find_node(&node) {
                n.remote_request()
            }

            // Validate the token
            let is_valid = match Token::new(a.token()) {
                Ok(t) => work_storage.token_store.checkin(IpAddr::from_socket_addr(addr), t),
                Err(_) => false,
            };

            // Create a socket address based on the implied/explicit port number
            let connect_addr = match a.connect_port() {
                ConnectPort::Implied => addr,
                ConnectPort::Explicit(port) => match addr {
                    SocketAddr::V4(v4_addr) => SocketAddr::V4(SocketAddrV4::new(*v4_addr.ip(), port)),
                    SocketAddr::V6(v6_addr) => SocketAddr::V6(SocketAddrV6::new(*v6_addr.ip(), port, v6_addr.flowinfo(), v6_addr.scope_id())),
                },
            };

            // Resolve type of response we are going to send
            let response_msg = if !is_valid {
                // Node gave us an invalid token
                warn!("bip_dht: Remote node sent us an invalid token for an AnnounceRequest...");
                ErrorMessage::new(
                    a.transaction_id().to_vec(),
                    ErrorCode::ProtocolError,
                    "Received An Invalid Token".to_owned(),
                )
                .encode()
            } else if work_storage.active_stores.add_item(a.info_hash(), connect_addr) {
                // Node successfully stored the value with us, send an announce response
                AnnouncePeerResponse::new(a.transaction_id(), work_storage.routing_table.node_id()).encode()
            } else {
                // Node unsuccessfully stored the value with us, send them an error message
                // TODO: Spec doesnt actually say what error message to send, or even if we should send one...
                warn!(
                    "bip_dht: AnnounceStorage failed to store contact information because it \
                       is full..."
                );
                ErrorMessage::new(a.transaction_id().to_vec(), ErrorCode::ServerError, "Announce Storage Is Full".to_owned()).encode()
            };

            if work_storage.out_channel.send((response_msg, addr)).is_err() {
                error!("bip_dht: Failed to send an announce peer response on the out channel...");
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
            }
        },
        Ok(MessageType::Response(ResponseType::FindNode(f))) => {
            info!("bip_dht: Received a FindNodeResponse...");
            let trans_id = TransactionID::from_bytes(f.transaction_id()).unwrap();
            let node = Node::as_good(f.node_id(), addr);

            // Add the payload nodes as questionable
            for (id, v4_addr) in f.nodes() {
                let sock_addr = SocketAddr::V4(v4_addr);

                work_storage.routing_table.add_node(Node::as_questionable(id, sock_addr));
            }

            let bootstrap_complete = {
                let opt_bootstrap = match table_actions.get_mut(&trans_id.action_id()) {
                    Some(&mut TableAction::Refresh(_)) => {
                        work_storage.routing_table.add_node(node);
                        None
                    },
                    Some(&mut TableAction::Bootstrap(ref mut bootstrap, ref mut attempts)) => {
                        if !bootstrap.is_router(&node.addr()) {
                            work_storage.routing_table.add_node(node);
                        }
                        Some((bootstrap, attempts))
                    },
                    Some(&mut TableAction::Lookup(_)) => {
                        error!("bip_dht: Resolved a FindNodeResponse ActionID to a TableLookup...");
                        None
                    },
                    None => {
                        error!(
                            "bip_dht: Resolved a TransactionID to a FindNodeResponse but no \
                                action found..."
                        );
                        None
                    },
                };

                if let Some((bootstrap, attempts)) = opt_bootstrap {
                    match bootstrap.recv_response(&trans_id, &work_storage.routing_table, &work_storage.out_channel, event_loop) {
                        BootstrapStatus::Idle => true,
                        BootstrapStatus::Bootstrapping => false,
                        BootstrapStatus::Failed => {
                            shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
                            false
                        },
                        BootstrapStatus::Completed => {
                            if should_rebootstrap(&work_storage.routing_table) {
                                attempt_rebootstrap(bootstrap, attempts, work_storage, event_loop) == Some(false)
                            } else {
                                true
                            }
                        },
                    }
                } else {
                    false
                }
            };

            if bootstrap_complete {
                broadcast_bootstrap_completed(trans_id.action_id(), table_actions, work_storage, event_loop);
            }

            if log_enabled!(LogLevel::Info) {
                let mut total = 0;

                for (index, bucket) in work_storage.routing_table.buckets().enumerate() {
                    let num_nodes = match bucket {
                        BucketContents::Empty => 0,
                        BucketContents::Sorted(b) => b.iter().filter(|n| n.status() == NodeStatus::Good).count(),
                        BucketContents::Assorted(b) => b.iter().filter(|n| n.status() == NodeStatus::Good).count(),
                    };
                    total += num_nodes;

                    if num_nodes != 0 {
                        print!("Bucket {}: {} | ", index, num_nodes);
                    }
                }

                print!("\nTotal: {}\n\n\n", total);
            }
        },
        Ok(MessageType::Response(ResponseType::GetPeers(g))) => {
            // info!("bip_dht: Received a GetPeersResponse...");
            let trans_id = TransactionID::from_bytes(g.transaction_id()).unwrap();
            let node = Node::as_good(g.node_id(), addr);

            work_storage.routing_table.add_node(node.clone());

            let opt_lookup = {
                match table_actions.get_mut(&trans_id.action_id()) {
                    Some(&mut TableAction::Lookup(ref mut lookup)) => Some(lookup),
                    Some(&mut TableAction::Refresh(_)) => {
                        error!(
                            "bip_dht: Resolved a GetPeersResponse ActionID to a \
                                TableRefresh..."
                        );
                        None
                    },
                    Some(&mut TableAction::Bootstrap(_, _)) => {
                        error!(
                            "bip_dht: Resolved a GetPeersResponse ActionID to a \
                                TableBootstrap..."
                        );
                        None
                    },
                    None => {
                        error!(
                            "bip_dht: Resolved a TransactionID to a GetPeersResponse but no \
                                action found..."
                        );
                        None
                    },
                }
            };

            if let Some(lookup) = opt_lookup {
                match lookup.recv_response(node, &trans_id, g, &work_storage.routing_table, &work_storage.out_channel, event_loop) {
                    LookupStatus::Searching => (),
                    LookupStatus::Completed => broadcast_dht_event(&mut work_storage.event_notifiers, DhtEvent::LookupCompleted(lookup.info_hash())),
                    LookupStatus::Failed => shutdown_event_loop(event_loop, ShutdownCause::Unspecified),
                    LookupStatus::Values(values) => {
                        for v4_addr in values {
                            let sock_addr = SocketAddr::V4(v4_addr);
                            work_storage.handshaker.connect(None, lookup.info_hash(), sock_addr);
                        }
                    },
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
        },
    }
}

fn handle_register_sender<H>(handler: &mut DhtHandler<H>, sender: mpsc::Sender<DhtEvent>) {
    handler.detached.event_notifiers.push(sender);
}

fn handle_start_bootstrap<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, routers: Vec<Router>, nodes: Vec<SocketAddr>)
where
    H: Handshaker,
{
    let (work_storage, table_actions) = (&mut handler.detached, &mut handler.table_actions);

    let router_iter = routers.into_iter().filter_map(|r| r.ipv4_addr().ok().map(SocketAddr::V4));

    let mid_generator = work_storage.aid_generator.generate();
    let action_id = mid_generator.action_id();
    let mut table_bootstrap = TableBootstrap::new(work_storage.routing_table.node_id(), mid_generator, nodes, router_iter);

    // Begin the bootstrap operation
    let bootstrap_status = table_bootstrap.start_bootstrap(&work_storage.out_channel, event_loop);

    work_storage.bootstrapping = true;
    table_actions.insert(action_id, TableAction::Bootstrap(table_bootstrap, 0));

    let bootstrap_complete = match bootstrap_status {
        BootstrapStatus::Idle => true,
        BootstrapStatus::Bootstrapping => false,
        BootstrapStatus::Failed => {
            shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
            false
        },
        BootstrapStatus::Completed => {
            // Check if our bootstrap was actually good
            if should_rebootstrap(&work_storage.routing_table) {
                let (bootstrap, attempts) = match table_actions.get_mut(&action_id) {
                    Some(&mut TableAction::Bootstrap(ref mut bootstrap, ref mut attempts)) => (bootstrap, attempts),
                    _ => panic!("bip_dht: Bug, in DhtHandler..."),
                };

                attempt_rebootstrap(bootstrap, attempts, work_storage, event_loop) == Some(false)
            } else {
                true
            }
        },
    };

    if bootstrap_complete {
        broadcast_bootstrap_completed(action_id, table_actions, work_storage, event_loop);
    }
}

fn handle_start_lookup<H>(
    table_actions: &mut HashMap<ActionID, TableAction>,
    work_storage: &mut DetachedDhtHandler<H>,
    event_loop: &mut EventLoop<DhtHandler<H>>,
    info_hash: InfoHash,
    should_announce: bool,
) where
    H: Handshaker,
{
    let mid_generator = work_storage.aid_generator.generate();
    let action_id = mid_generator.action_id();

    if work_storage.bootstrapping {
        // Queue it up if we are currently bootstrapping
        work_storage.future_actions.push(PostBootstrapAction::Lookup(info_hash, should_announce));
    } else {
        // Start the lookup right now if not bootstrapping
        match TableLookup::new(
            work_storage.routing_table.node_id(),
            info_hash,
            mid_generator,
            should_announce,
            &work_storage.routing_table,
            &work_storage.out_channel,
            event_loop,
        ) {
            Some(lookup) => {
                table_actions.insert(action_id, TableAction::Lookup(lookup));
            },
            None => shutdown_event_loop(event_loop, ShutdownCause::Unspecified),
        }
    }
}

fn handle_shutdown<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, cause: ShutdownCause)
where
    H: Handshaker,
{
    let (work_storage, _) = (&mut handler.detached, &mut handler.table_actions);

    broadcast_dht_event(&mut work_storage.event_notifiers, DhtEvent::ShuttingDown(cause));

    event_loop.shutdown();
}

fn handle_check_table_refresh<H>(
    table_actions: &mut HashMap<ActionID, TableAction>,
    work_storage: &mut DetachedDhtHandler<H>,
    event_loop: &mut EventLoop<DhtHandler<H>>,
    trans_id: TransactionID,
) where
    H: Handshaker,
{
    let opt_refresh_status = match table_actions.get_mut(&trans_id.action_id()) {
        Some(&mut TableAction::Refresh(ref mut refresh)) => {
            Some(refresh.continue_refresh(&work_storage.routing_table, &work_storage.out_channel, event_loop))
        },
        Some(&mut TableAction::Lookup(_)) => {
            error!(
                "bip_dht: Resolved a TransactionID to a check table refresh but TableLookup \
                    found..."
            );
            None
        },
        Some(&mut TableAction::Bootstrap(_, _)) => {
            error!(
                "bip_dht: Resolved a TransactionID to a check table refresh but \
                    TableBootstrap found..."
            );
            None
        },
        None => {
            error!(
                "bip_dht: Resolved a TransactionID to a check table refresh but no action \
                    found..."
            );
            None
        },
    };

    match opt_refresh_status {
        None => (),
        Some(RefreshStatus::Refreshing) => (),
        Some(RefreshStatus::Failed) => shutdown_event_loop(event_loop, ShutdownCause::Unspecified),
    }
}

fn handle_check_bootstrap_timeout<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, trans_id: TransactionID)
where
    H: Handshaker,
{
    let (work_storage, table_actions) = (&mut handler.detached, &mut handler.table_actions);

    let bootstrap_complete = {
        let opt_bootstrap_info = match table_actions.get_mut(&trans_id.action_id()) {
            Some(&mut TableAction::Bootstrap(ref mut bootstrap, ref mut attempts)) => Some((
                bootstrap.recv_timeout(&trans_id, &work_storage.routing_table, &work_storage.out_channel, event_loop),
                bootstrap,
                attempts,
            )),
            Some(&mut TableAction::Lookup(_)) => {
                error!(
                    "bip_dht: Resolved a TransactionID to a check table bootstrap but \
                        TableLookup found..."
                );
                None
            },
            Some(&mut TableAction::Refresh(_)) => {
                error!(
                    "bip_dht: Resolved a TransactionID to a check table bootstrap but \
                        TableRefresh found..."
                );
                None
            },
            None => {
                error!(
                    "bip_dht: Resolved a TransactionID to a check table bootstrap but no \
                        action found..."
                );
                None
            },
        };

        match opt_bootstrap_info {
            None => false,
            Some((BootstrapStatus::Idle, _, _)) => true,
            Some((BootstrapStatus::Bootstrapping, _, _)) => false,
            Some((BootstrapStatus::Failed, _, _)) => {
                shutdown_event_loop(event_loop, ShutdownCause::Unspecified);
                false
            },
            Some((BootstrapStatus::Completed, bootstrap, attempts)) => {
                // Check if our bootstrap was actually good
                if should_rebootstrap(&work_storage.routing_table) {
                    attempt_rebootstrap(bootstrap, attempts, work_storage, event_loop) == Some(false)
                } else {
                    true
                }
            },
        }
    };

    if bootstrap_complete {
        broadcast_bootstrap_completed(trans_id.action_id(), table_actions, work_storage, event_loop);
    }
}

fn handle_check_lookup_timeout<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, trans_id: TransactionID)
where
    H: Handshaker,
{
    let (work_storage, table_actions) = (&mut handler.detached, &mut handler.table_actions);

    let opt_lookup_info = match table_actions.get_mut(&trans_id.action_id()) {
        Some(&mut TableAction::Lookup(ref mut lookup)) => Some((
            lookup.recv_timeout(&trans_id, &work_storage.routing_table, &work_storage.out_channel, event_loop),
            lookup.info_hash(),
        )),
        Some(&mut TableAction::Bootstrap(_, _)) => {
            error!(
                "bip_dht: Resolved a TransactionID to a check table lookup but TableBootstrap \
                    found..."
            );
            None
        },
        Some(&mut TableAction::Refresh(_)) => {
            error!(
                "bip_dht: Resolved a TransactionID to a check table lookup but TableRefresh \
                    found..."
            );
            None
        },
        None => {
            error!(
                "bip_dht: Resolved a TransactionID to a check table lookup but no action \
                    found..."
            );
            None
        },
    };

    match opt_lookup_info {
        None => (),
        Some((LookupStatus::Searching, _)) => (),
        Some((LookupStatus::Completed, info_hash)) => broadcast_dht_event(&mut work_storage.event_notifiers, DhtEvent::LookupCompleted(info_hash)),
        Some((LookupStatus::Failed, _)) => shutdown_event_loop(event_loop, ShutdownCause::Unspecified),
        Some((LookupStatus::Values(v), info_hash)) => {
            // Add values to handshaker
            for v4_addr in v {
                let sock_addr = SocketAddr::V4(v4_addr);

                work_storage.handshaker.connect(None, info_hash, sock_addr);
            }
        },
    }
}

fn handle_check_lookup_endgame<H>(handler: &mut DhtHandler<H>, event_loop: &mut EventLoop<DhtHandler<H>>, trans_id: TransactionID)
where
    H: Handshaker,
{
    let (work_storage, table_actions) = (&mut handler.detached, &mut handler.table_actions);

    let opt_lookup_info = match table_actions.remove(&trans_id.action_id()) {
        Some(TableAction::Lookup(mut lookup)) => Some((
            lookup.recv_finished(work_storage.handshaker.port(), &work_storage.routing_table, &work_storage.out_channel),
            lookup.info_hash(),
        )),
        Some(TableAction::Bootstrap(_, _)) => {
            error!(
                "bip_dht: Resolved a TransactionID to a check table lookup but TableBootstrap \
                    found..."
            );
            None
        },
        Some(TableAction::Refresh(_)) => {
            error!(
                "bip_dht: Resolved a TransactionID to a check table lookup but TableRefresh \
                    found..."
            );
            None
        },
        None => {
            error!(
                "bip_dht: Resolved a TransactionID to a check table lookup but no action \
                    found..."
            );
            None
        },
    };

    match opt_lookup_info {
        None => (),
        Some((LookupStatus::Searching, _)) => (),
        Some((LookupStatus::Completed, info_hash)) => broadcast_dht_event(&mut work_storage.event_notifiers, DhtEvent::LookupCompleted(info_hash)),
        Some((LookupStatus::Failed, _)) => shutdown_event_loop(event_loop, ShutdownCause::Unspecified),
        Some((LookupStatus::Values(v), info_hash)) => {
            // Add values to handshaker
            for v4_addr in v {
                let sock_addr = SocketAddr::V4(v4_addr);

                work_storage.handshaker.connect(None, info_hash, sock_addr);
            }
        },
    }
}
