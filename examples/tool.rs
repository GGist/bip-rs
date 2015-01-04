extern crate "rust-bt" as rust_bt;
extern crate "crypto" as rust_crypto;
extern crate serialize;

use std::io::fs::File;
use std::io::net::ip::{SocketAddr, Ipv4Addr};
use serialize::hex::ToHex;
use rust_crypto::sha1::Sha1;
use rust_crypto::digest::Digest;
use rust_bt::bencode::Bencode;
use rust_bt::tracker::udp::UdpTracker;
use rust_bt::tracker::Tracker;
use rust_bt::torrent::{Torrent};
use rust_bt::upnp::{UPnPIntf, ServiceDesc};

fn main() {
    check();
    //forward_port();
}

fn check() {
    let upnp = UPnPIntf::find_all(SocketAddr{ ip: Ipv4Addr(192, 168, 1, 102), port: 2000 }).unwrap();
    
    for i in upnp.iter() {
        println!("{} {}\n", i.usn(), i.location());
    }
}

fn forward_port() {
    let upnp = UPnPIntf::find_services(SocketAddr{ ip: Ipv4Addr(192, 168, 1, 102), port: 2000 }, 
        "WANIPConnection", "1")
    .unwrap();
    let service = upnp[0].service_desc().unwrap();
    
    service.send_action("AddPortMapping", &[("NewRemoteHost", ""),
        ("NewExternalPort", "6882"),
        ("NewProtocol", "TCP"),
        ("NewInternalPort", "6882"),
        ("NewInternalClient", "192.168.1.102"),
        ("NewEnabled", "1"),
        ("NewPortMappingDescription", "RustBT"),
        ("NewLeaseDuration", "0")])
    .unwrap();
}

fn scrape_torrent() {
    let mut torr_file = File::open(&Path::new("tests/data/test.torrent"));
    let torr_bytes = torr_file.read_to_end().unwrap();
    let ben_val = Bencode::new(torr_bytes.as_slice()).unwrap();
    let encoded = ben_val.dict().unwrap().get("info").unwrap().encoded();
    let torrent = Torrent::new(&ben_val).unwrap();

    let mut sha = Sha1::new();
    let mut result = [0u8; 20];
	
    sha.input(encoded.as_slice());
    sha.result(result.as_mut_slice());
	
    let name = torrent.comment();
    println!("{}", result.to_hex());
    
    let mut tracker = UdpTracker::new(torrent.announce(), &result).unwrap();
    let scrape = tracker.scrape().unwrap();
    println!("{} {} {} {}", name, scrape.leechers, scrape.seeders, scrape.downloads);
    println!("{}", tracker.local_ip().unwrap()); 
}