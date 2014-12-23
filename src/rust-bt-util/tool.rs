extern crate "rust-bt" as rust_bt;
//extern crate crypto;   
extern crate serialize;

fn main() {
    use std::io::fs::File;
    
    use serialize::hex::ToHex;
    //use crypto::sha1::Sha1;
    //use crypto::digest::Digest;
    use rust_bt::bencode::Bencode;
    use rust_bt::tracker_udp::UdpTracker;
    use rust_bt::tracker::Tracker;
    use rust_bt::torrent::{Torrent};
    
    let mut torr_file = File::open(&Path::new("tests/data/good_udp.torrent"));
    let torr_bytes = torr_file.read_to_end().unwrap();
    let ben_val = Bencode::new(torr_bytes.as_slice()).unwrap();
    let encoded = ben_val.dict().unwrap().get("info").unwrap().encoded();
    let torrent = Torrent::new(&ben_val).unwrap();
    
    //let mut sha = Sha1::new();
    let mut result = [0u8,..20];
	
    //sha.input(encoded.as_slice());
    //sha.result(result.as_mut_slice());
	
    let (_, name) = torrent.file_type();
    //println!("{}", result.to_hex());
    
    //let mut tracker = UdpTracker::new(torrent.announce(), &result).unwrap();
    //let scrape = tracker.scrape().unwrap();
    //println!("{} {} {} {}", name, scrape.leechers, scrape.seeders, scrape.downloads);
    //println!("{}", tracker.local_ip().unwrap()); 
}