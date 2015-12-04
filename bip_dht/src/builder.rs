use std::collections::{HashSet};
use std::io::{self};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{self, Receiver};

use bip_handshake::{Handshaker};
use bip_util::bt::{InfoHash};
use bip_util::net::{self};
use mio::{Sender};

use router::{Router};
use worker::{self, OneshotTask, DhtEvent, ShutdownCause};

/// Maintains a distributed hash (routing) table.
pub struct MainlineDht {
    send: Sender<OneshotTask>
}

impl MainlineDht {
    pub fn with_builder<H>(builder: DhtBuilder, handshaker: H) -> io::Result<MainlineDht>
        where H: Handshaker + 'static {
        let send_sock = try!(UdpSocket::bind(&builder.src_addr));
        let recv_sock = try!(send_sock.try_clone());
        
        let kill_sock = try!(send_sock.try_clone());
        let kill_addr = try!(send_sock.local_addr());
        
        let send = try!(worker::start_mainline_dht(send_sock, recv_sock, builder.read_only,
            builder.ext_addr, handshaker, kill_sock, kill_addr));
        
        let nodes: Vec<SocketAddr> = builder.nodes.into_iter().collect();
        let routers: Vec<Router> = builder.routers.into_iter().collect();
        
        if send.send(OneshotTask::StartBootstrap(routers, nodes)).is_err() {
            warn!("bip_dt: MainlineDht failed to send a start bootstrap message...");
        }
        
        Ok(MainlineDht{ send: send })
    }
    
    pub fn search(&self, hash: InfoHash, announce: bool) {
        if self.send.send(OneshotTask::StartLookup(hash, announce)).is_err() {
            warn!("bip_dht: MainlineDht failed to send a start lookup message...");
        }
    }
    
    pub fn events(&self) -> Receiver<DhtEvent> {
        let (send, recv) = mpsc::channel();
        
        if self.send.send(OneshotTask::RegisterSender(send)).is_err() {
            warn!("bip_dht: MainlineDht failed to send a register sender message...");
        }
        
        recv
    }
}

impl Drop for MainlineDht {
    fn drop(&mut self) {
        if self.send.send(OneshotTask::Shutdown(ShutdownCause::ClientInitiated)).is_err() {
            warn!("bip_dht: MainlineDht failed to send a shutdown message...");
        }
    }
}

//----------------------------------------------------------------------------//

/// Stores information for initializing a dht.
#[derive(Clone, Debug)] 
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
            read_only: true,
            src_addr: net::default_route_v4(),
            ext_addr: None
        }
    }

    /// Creates a DhtBuilder with an initial node for our routing table.
    pub fn with_node(node_addr: SocketAddr) -> DhtBuilder {
        let dht = DhtBuilder::new();
        
        dht.add_node(node_addr)
    }
    
    /// Creates a DhtBuilder with an initial router which will let us gather nodes
    /// if our routing table is ever empty.
    ///
    /// Difference between a node and a router is that a router is never put in
    /// our routing table.
    pub fn with_router(router: Router) -> DhtBuilder {
        let dht = DhtBuilder::new();
        
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
    /// incoming network traffic. Defaults value is true.
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
    pub fn start_mainline<H>(self, handshaker: H) -> io::Result<MainlineDht>
        where H: Handshaker + 'static {
        MainlineDht::with_builder(self, handshaker)
    }
}