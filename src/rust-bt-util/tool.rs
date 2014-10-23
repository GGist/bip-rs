#![feature(macro_rules)]
#![feature(phase)]

#[phase(plugin)]

extern crate regex_macros;
extern crate regex;
extern crate serialize;
extern crate "rust-bt" as rust_bt;
extern crate "rust-crypto" as crypto;

macro_rules! check(
    ($e:expr) => (match $e { Some(e) => e, None => { println!("Found None"); return }})
)

fn main() {
    use std::io::{IoResult};
    use std::io::net::ip::{SocketAddr, Ipv4Addr, Ipv6Addr, IpAddr};
    use std::io::net::udp::{UdpSocket};
    use std::io::fs::File;
    use std::io::net::addrinfo::get_host_addresses;
    use std::u16;

    use serialize::hex::ToHex;
    use crypto::sha1::Sha1;
    use crypto::digest::Digest;
    use rust_bt::bencode::BenVal;
    use rust_bt::tracker_udp::UdpTracker;
    use rust_bt::tracker::Tracker;
    use rust_bt::torrent::{Torrent};
    use rust_bt::upnp::UPnP;
    
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
    
    let dict = check!(ben_val.dict());
    
    let announce_url = check!(check!(dict.find_equiv(&"announce")).str());
    
    let mut sha = Sha1::new();
    let mut result = [0u8,..20];
    let encoded = check!(dict.find_equiv(&"info")).encoded();
    
    sha.input(encoded.as_slice());
    sha.result(result);

    println!("{}", torrent.announce);
    
    //let data = UPnP::search();
    //for i in data.iter() {
    //    println!("{}", i.data);
    //}
    
    //println!("{}", result.to_hex());
    
    let mut conn = UdpTracker::new(announce_url, result).unwrap();
    //let bytes = conn.announce(5).unwrap();
    
    //println!("{}", bytes.seeders);
    //println!("{}", bytes.leechers);
    
    //for &i in bytes.peers.iter() {
    //    println!("{}", i);
    //}
}