extern crate bittorrent;
extern crate crypto;

use std::io::fs::{File};
use crypto::sha1::{Sha1};
use crypto::digest::{Digest};
use bittorrent::bencode::{Bencode};
use bittorrent::tracker::udp::{UdpTracker};
use bittorrent::tracker::{Tracker};
use bittorrent::torrent::{Torrent};

fn main() {
    let mut torr_file = File::open(&Path::new("tests/data/test.torrent"));
    let torr_bytes = torr_file.read_to_end().unwrap();
    let ben_val = Bencode::new(torr_bytes.as_slice()).unwrap();
    
    let info_dict = ben_val.dict().unwrap().get("info")
        .unwrap().encoded();
    let torrent = Torrent::new(&ben_val).unwrap();
    let (_, name) = torrent.file_type();
    
    let mut sha = Sha1::new();
    let mut result = [0u8; 20];
    sha.input(info_dict.as_slice());
    sha.result(result.as_mut_slice());
    
    let mut tracker = UdpTracker::new(torrent.announce(), &result)
        .unwrap();
    let scrape_response = tracker.scrape().unwrap();
    
    println!("Torrent Name:{} Leechers:{} Seeders:{} Total Downloads:{}", 
        name, 
        scrape_response.leechers, 
        scrape_response.seeders, 
        scrape_response.downloads
    );
}