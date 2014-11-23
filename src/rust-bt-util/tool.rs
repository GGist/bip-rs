extern crate serialize;
extern crate "rust-bt" as rust_bt;
extern crate "rust-crypto" as crypto;   

fn main() {
    use std::io::fs::File;

    use crypto::sha1::Sha1;
    use crypto::digest::Digest;
    use rust_bt::bencode::BenVal;
    use rust_bt::tracker_udp::UdpTracker;
    use rust_bt::tracker::Tracker;
    use rust_bt::torrent::{Torrent};
    use rust_bt::upnp::UPnPIntf;
    use rust_bt::upnp::ServiceDesc;
    
    let mut torr_file = File::open(&Path::new("tests/data/udp_tracker/sample.torrent"));
    let torr_bytes = match torr_file.read_to_end() {
        Ok(n)  => n,
        Err(n) => { println!("{}", n); return }
    };
    
    let ben_val: BenVal = match BenVal::new(torr_bytes.as_slice()) {
        Ok(n) => n,
        Err(n) => { println!("{}", n); return }
    };
    let torrent = Torrent::new(&ben_val);
    
    if torrent.is_err() {
        println!("{}", torrent.err().unwrap());
        return;
    }
    let torrent = torrent.unwrap();
    
    let dict = ben_val.dict().expect("1");
    
    let announce_url = dict.get("announce").expect("2").str().expect("3");
    
    let mut sha = Sha1::new();
    let mut result = [0u8,..20];
    let encoded = dict.get("info").expect("4").encoded();
    
    sha.input(encoded.as_slice());
    sha.result(result.as_mut_slice());
    println!("{}", announce_url);
    let mut tracker = UdpTracker::new(announce_url, &result).unwrap();
    let scrape = tracker.scrape().unwrap();
    println!("{} {} {}", scrape.leechers, scrape.seeders, scrape.downloads);
    println!("{}", tracker.local_ip().unwrap());
}