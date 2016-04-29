extern crate bip_utracker;
extern crate bip_util;

use std::collections::{HashSet, HashMap};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Sender};

use bip_util::bt::{InfoHash, PeerId};
use bip_util::trans::{TIDGenerator};
use bip_utracker::{ServerHandler, ServerResult, Handshaker, ClientMetadata};
use bip_utracker::announce::{AnnounceResponse, AnnounceRequest, AnnounceEvent};
use bip_utracker::contact::{CompactPeersV4, CompactPeersV6, CompactPeers};
use bip_utracker::scrape::{ScrapeRequest, ScrapeResponse, ScrapeStats};

mod test_announce_start;
mod test_announce_stop;
mod test_client_drop;
mod test_client_full;
mod test_connect;
mod test_connect_cache;
mod test_scrape;
mod test_server_drop;

const NUM_PEERS_RETURNED: usize = 20;

#[derive(Clone)]
struct MockTrackerHandler {
    inner: Arc<Mutex<InnerMockTrackerHandler>>
}

struct InnerMockTrackerHandler {
    cids:          HashSet<u64>,
    cid_generator: TIDGenerator<u64>,
    peers_map:     HashMap<InfoHash, HashSet<SocketAddr>>
}

impl MockTrackerHandler {
    pub fn new() -> MockTrackerHandler {
        MockTrackerHandler{ inner: Arc::new(Mutex::new(InnerMockTrackerHandler{ 
            cids: HashSet::new(), cid_generator: TIDGenerator::<u64>::new(),
            peers_map: HashMap::new() })) }
    }
    
    pub fn num_active_connect_ids(&self) -> usize {
        self.inner.lock().unwrap().cids.len()
    }
}

impl ServerHandler for MockTrackerHandler {
    fn connect<R>(&mut self, _: SocketAddr, result: R)
        where R: for<'a> FnOnce(ServerResult<'a, u64>) {
        let mut inner_lock = self.inner.lock().unwrap();
            
        let cid = inner_lock.cid_generator.generate();
        inner_lock.cids.insert(cid);
        
        result(Ok(cid));
    }
    
    fn announce<'b, R>(&mut self, addr: SocketAddr, id: u64, req: &AnnounceRequest<'b>, result: R)
        where R: for<'a> FnOnce(ServerResult<'a, AnnounceResponse<'a>>) {
        let mut inner_lock = self.inner.lock().unwrap();
            
        if inner_lock.cids.contains(&id) {
            let peers = inner_lock.peers_map.entry(req.info_hash()).or_insert(HashSet::new());
            // Ignore any source ip directives in the request
            let store_addr = match addr {
                SocketAddr::V4(v4_addr) => SocketAddr::V4(SocketAddrV4::new(*v4_addr.ip(), req.port())),
                SocketAddr::V6(v6_addr) => SocketAddr::V6(SocketAddrV6::new(*v6_addr.ip(), req.port(), 0, 0))
            };
            
            // Resolve what to do with the event
            match req.state().event() {
                AnnounceEvent::None      => peers.insert(store_addr),
                AnnounceEvent::Completed => peers.insert(store_addr),
                AnnounceEvent::Started   => peers.insert(store_addr),
                AnnounceEvent::Stopped   => peers.remove(&store_addr)
            };
            
            // Check what type of peers the request warrants
            let compact_peers = if req.source_ip().is_ipv4() {
                let mut v4_peers = CompactPeersV4::new();
                
                for v4_addr in peers.iter()
                    .filter_map(|addr| {
                        match addr {
                            &SocketAddr::V4(v4_addr) => Some(v4_addr),
                            &SocketAddr::V6(_)       => None
                        }
                    })
                    .take(NUM_PEERS_RETURNED) {
                    v4_peers.insert(v4_addr);
                }
                
                CompactPeers::V4(v4_peers)
            } else {
                let mut v6_peers = CompactPeersV6::new();
                
                for v6_addr in peers.iter()
                    .filter_map(|addr| {
                        match addr {
                            &SocketAddr::V4(_)       => None,
                            &SocketAddr::V6(v6_addr) => Some(v6_addr)
                        }
                    })
                    .take(NUM_PEERS_RETURNED) {
                    v6_peers.insert(v6_addr);
                }
                
                CompactPeers::V6(v6_peers)
            };
            
            result(Ok(AnnounceResponse::new(1800, peers.len() as i32, peers.len() as i32, compact_peers)));
        } else {
            result(Err("Connection ID Is Invalid"));
        }
    }
        
    fn scrape<'b, R>(&mut self, _: SocketAddr, id: u64, req: &ScrapeRequest<'b>, result: R)
        where R: for<'a> FnOnce(ServerResult<'a, ScrapeResponse<'a>>) {
        let mut inner_lock = self.inner.lock().unwrap();
        
        if inner_lock.cids.contains(&id) {
            let mut response = ScrapeResponse::new();
            
            for hash in req.iter() {
                let peers = inner_lock.peers_map.entry(hash).or_insert(HashSet::new());
                
                response.insert(ScrapeStats::new(peers.len() as i32, 0, peers.len() as i32));
            }
            
            result(Ok(response));
        } else {
            result(Err("Connection ID Is Invalid"));
        }
    }
}

//----------------------------------------------------------------------------//

#[derive(Clone)]
struct MockHandshaker {
    send:     Sender<ClientMetadata>,
    connects: Arc<Mutex<Vec<SocketAddr>>>
}

impl MockHandshaker {
    pub fn new(send: Sender<ClientMetadata>) -> MockHandshaker {
        MockHandshaker{ send: send, connects: Arc::new(Mutex::new(Vec::new())) }
    }
    
    pub fn connects_received<F>(&self, callback: F)
        where F: FnOnce(&[SocketAddr]) {
        let locked = self.connects.lock().unwrap();
        
        callback(&*locked);
    }
}

impl Handshaker for MockHandshaker {
    type MetadataEnvelope = ClientMetadata;
    
    fn id(&self) -> PeerId {
        [0u8; 20].into()
    }
    
    fn port(&self) -> u16 {
        6969
    }
    
    fn connect(&mut self, _: Option<PeerId>, _: InfoHash, addr: SocketAddr) {
        self.connects.lock().unwrap().push(addr);
    }
    
    fn metadata(&mut self, data: ClientMetadata) {
        self.send.send(data).unwrap();
    }
}