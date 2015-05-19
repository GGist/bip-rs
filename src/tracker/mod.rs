//! Unified interface for communicating with different trackers.

use std::old_io::{IoResult};
use std::old_io::net::ip::{SocketAddr, IpAddr};

use types::{Timepoint};

pub mod udp;

/// Information pertaining to the swarm we are in.
pub struct AnnounceInfo {
    /// Number of leechers in the swarm.
    pub leechers: i32,
    /// Number of seeders in the swarm.
    pub seeders:  i32,
    /// List of SocketAddrs for remote peers in the swarm.
    pub peers:    Vec<SocketAddr>,
    /// Indicates when to send an update to the tracker.
    pub interval: Timepoint
}

/// Statistics for a specific torrent.
#[derive(Copy)]
pub struct ScrapeInfo {
    /// Number of leechers in the swarm.
    pub leechers:  i32,
    /// Number of seeders in the swarm.
    pub seeders:   i32,
    /// Number of downloads for this torrent.
    pub downloads: i32
}

/// Statistics for our download session.
#[derive(Copy)]
pub struct TransferStatus {
    /// Number of bytes downloaded so far.
    pub downloaded: i64,
    /// Number of bytes left to download.
    pub remaining:  i64,
    /// Number of bytes uploaded so far.
    pub uploaded:   i64
}

/// Interface for communicating with an generic tracker.
pub trait Tracker {
    /// Returns the local ip address that is being used to communicate with the tracker.
    fn local_ip(&mut self) -> IpAddr;

    /// Returns information about the swarm for a particular torrent file without
    /// joining the swarm.
    ///
    /// This is a blocking operation.
    fn send_scrape(&mut self) -> IoResult<ScrapeInfo>;
    
    /// Sends an announce request to the tracker signalling a start event. This request 
    /// enters us into the swarm and we are required to send periodic updates as 
    /// specified by the tracker in order to be kept in the swarm. Periodic updates 
    /// should be sent with update_announce.
    ///
    /// This is a blocking operation.
    fn start_announce(&mut self, remaining: i64) -> IoResult<AnnounceInfo>;
    
    /// Sends an announce request to the tracker signalling an update event. This request
    /// acts as a heartbeat so that the tracker knows we are still connected and wanting
    /// to be kept in the swarm.
    ///
    /// This is a blocking operation.
    fn update_announce(&mut self, status: TransferStatus) -> IoResult<AnnounceInfo>;
    
    /// Sends an announce request to the tracker signalling a stop event. This request
    /// exists to let the tracker know that we are gracefully shutting down and that
    /// it should remove us from the swarm.
    ///
    /// This is a blocking operation.
    fn stop_announce(&mut self, status: TransferStatus) -> IoResult<()>;
    
    /// Sends an announce request to the tracker signalling a completed event. This request
    /// exists to let the tracker know that we have completed our download and wish to
    /// remain in the swarm as a seeder.
    ///
    /// This is a blocking operation.
    fn complete_announce(&mut self, status: TransferStatus) -> IoResult<AnnounceInfo>;
}