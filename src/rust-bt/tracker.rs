use std::io::{IoResult};
use std::io::net::ip::{SocketAddr, IpAddr};

#[deriving(Copy)]
pub struct ScrapeInfo {
    pub leechers: i32,
    pub seeders: i32,
    pub downloads: i32
}

pub struct AnnounceInfo {
    pub interval: Receiver<()>,
    pub leechers: i32,
    pub seeders: i32,
    pub peers: Vec<SocketAddr> 
    // TODO: Better interface should allow for Vec reuse on updates. As of now, we
    // are allocating a new Vec for each update_announce to store a peer list that
    // has most likely, not changed or added/removed a few peers.
}

pub trait Tracker {
    /// Returns the local ip address that is being used to communicate with the tracker.
    fn local_ip(&mut self) -> IoResult<IpAddr>;

    /// Returns information about the swarm for a particular torrent file without
    /// joining the swarm.
    ///
    /// This is a blocking operation.
    fn scrape(&mut self) -> IoResult<ScrapeInfo>;
    
    /// Sends an announce request to the tracker signalling a start event. This request 
    /// enters us into the swarm and we are required to send periodic updates as 
    /// specified by the tracker in order to be kept in the swarm. Periodic updates 
    /// should be sent with update_announce.
    ///
    /// This is a blocking operation.
    fn start_announce(&mut self, total_bytes: uint) -> IoResult<AnnounceInfo>;
    
    /// Sends an announce request to the tracker signalling an update event. This request
    /// acts as a heartbeat so that the tracker knows we are still connected and wanting
    /// to be kept in the swarm.
    ///
    /// This is a blocking operation.
    fn update_announce(&mut self, total_down: uint, total_left: uint, total_up: uint) -> IoResult<AnnounceInfo>;
    
    /// Sends an announce request to the tracker signalling a stop event. This request
    /// exists to let the tracker know that we are gracefully shutting down and that
    /// it should remove us from the swarm.
    ///
    /// This is a blocking operation.
    fn stop_announce(&mut self, total_down: uint, total_left: uint, total_up: uint) -> IoResult<()>;
    
    /// Sends an announce request to the tracker signalling a completed event. This request
    /// exists to let the tracker know that we have completed our download TEST TO CHECK
    /// WHAT EXACTLY THIS MAKES THE TRACKER DO.
    ///
    /// This is a blocking operation.
    fn complete_announce(&mut self, total_bytes: uint) -> IoResult<()>;
}