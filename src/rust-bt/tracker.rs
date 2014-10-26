use std::io::{IoResult};
use std::io::net::ip::{SocketAddr};

pub struct AnnounceInfo {
    pub interval: i32,
    pub leechers: i32,
    pub seeders: i32,
    pub peers: Vec<SocketAddr>
}

pub trait Tracker {
    fn socket_name(&mut self) -> IoResult<SocketAddr>;

    fn announce(&mut self, total_bytes: uint) -> IoResult<AnnounceInfo>;
    
    //fn announce_update(&mut self, );
    
    //fn announce_stop();
    
    //fn announce_complete();
    // fn update(&mut self, bytes_received: uint, bytes_left: uint) -> IoResult<Vec<u8>>;
    
    //pub fn stop(&mut self, bytes_received: uint, bytes_left: uint) -> IoResult<Vec<u8>>;
    
    //pub fn complete(&mut self, bytes_received: uint, bytes_left: uint) -> IoResult<Vec<u8>>;
}