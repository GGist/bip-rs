use std::collections::{HashSet};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool};
use std::sync::mpsc::{self};

use bip_handshake::{Handshaker};
use bip_util::{self, InfoHash};
use mio::{Sender};

use error::{DhtResult, DhtError, DhtErrorKind};
use router::{Router};
use worker::{self, OneshotTask, ScheduledTask};

/// Maintains a distributed hash (routing) table.
pub struct MainlineDht {
    send: Sender<OneshotTask>
}

// Starting the dht, ping nodes that were added, if they respond add them to the dht
// If no nodes were added, use the router that was provided

impl MainlineDht {
    pub fn with_builder<H>(builder: DhtBuilder, handshaker: H) -> DhtResult<MainlineDht>
        where H: Handshaker + 'static {
        let send_socket = try!(UdpSocket::bind(&builder.src_addr));
        let recv_socket = try!(send_socket.try_clone());
        
        let send = try!(worker::start_mainline_dht(send_socket, recv_socket, builder.read_only, builder.ext_addr, handshaker));
        
        let nodes: Vec<SocketAddr> = builder.nodes.into_iter().collect();
        let routers: Vec<Router> = builder.routers.into_iter().collect();
        
        send.send(OneshotTask::StartBootstrap(routers, nodes));
        //send.send(OneshotTask::ScheduleTask(1000, IntervalTask::CheckBootstrap(0)));
        
        Ok(MainlineDht{ send: send })
    }
    
    pub fn search(&self, hash: InfoHash) -> DhtResult<()> {
        let (send, recv) = mpsc::sync_channel(1);
        
        if self.send.send(OneshotTask::StartLookup(hash, send)).is_ok() {
            recv.recv();
            
            Ok(())
        } else {
            Err(DhtError::new(DhtErrorKind::LookupFailed, "Failed To Send A Message To The DhtHandler..."))
        }
    }
    
    pub fn announce(hash: InfoHash) -> DhtResult<()> {
        unimplemented!();
    }
}

//----------------------------------------------------------------------------//

/// Stores information for initializing a dht.
#[derive(Clone, PartialEq, Eq, Debug)] 
pub struct DhtBuilder {
    nodes:     HashSet<SocketAddr>,
    routers:   HashSet<Router>,
    read_only: bool,
    src_addr:  SocketAddr,
    ext_addr:  Option<SocketAddr>
}
    
impl DhtBuilder {
    /// Create a new DhtBuilder.
    ///
    /// This should not be used directly, force the user to supply builder with
    /// some initial bootstrap method.
    fn new() -> DhtBuilder {
        DhtBuilder{ nodes: HashSet::new(),
            routers: HashSet::new(),
            read_only: false,
            src_addr: bip_util::default_route_v4(),
            ext_addr: None
        }
    }

    /// Creates a DhtBuilder with an initial node for our routing table.
    pub fn with_node(node_addr: SocketAddr) -> DhtBuilder {
        let mut dht = DhtBuilder::new();
        
        dht.add_node(node_addr)
    }
    
    /// Creates a DhtBuilder with an initial router which will let us gather nodes
    /// if our routing table is ever empty.
    ///
    /// Difference between a node and a router is that a router is never put in
    /// our routing table.
    pub fn with_router(router: Router) -> DhtBuilder {
        let mut dht = DhtBuilder::new();
        
        dht.add_router(router)
    }

    /// Add nodes which will be distributed within our routing table.
    pub fn add_node(mut self, node_addr: SocketAddr) -> DhtBuilder {
        self.nodes.insert(node_addr);
        
        self
    }

    /// Add a router which will let us gather nodes if our routing table is ever empty.
    ///
    /// See DhtBuilder::with_router for difference between a router and a node.
    pub fn add_router(mut self, router: Router) -> DhtBuilder {
        self.routers.insert(router);
        
        self
    }

    /// Set the read only flag when communicating with other nodes. Indicates
    /// that remote nodes should not add us to their routing table.
    ///
    /// Used when we are behind a restrictive NAT and/or we want to decrease
    /// incoming network traffic.
    pub fn set_read_only(mut self, read_only: bool) -> DhtBuilder {
        self.read_only = read_only;
        
        self
    }
    
    /// Provide the dht with our external address. If this is not supplied we will
    /// have to deduce this information from remote nodes.
    ///
    /// Purpose of the external address is to generate a node id the conforms to
    /// BEP 42 so that nodes can safely store information on our node.
    pub fn set_external_addr(mut self, addr: SocketAddr) -> DhtBuilder {
        self.ext_addr = Some(addr);
        
        self
    }
    
    /// Provide the dht with the source address.
    ///
    /// If this is not supplied we will use the OS default route.
    pub fn set_source_addr(mut self, addr: SocketAddr) -> DhtBuilder {
        self.src_addr = addr;
    
        self
    }
    
    /// Start a mainline dht with the current configuration.
    pub fn start_mainline<H>(self, handshaker: H) -> DhtResult<MainlineDht>
        where H: Handshaker + 'static {
        MainlineDht::with_builder(self, handshaker)
    }
}