#![allow(unstable)]

extern crate bittorrent;

use std::io::net::ip::{SocketAddr, Ipv4Addr};
use bittorrent::upnp::{UPnPIntf};

// Fill In With Local Address
static LOCAL_ADDR: SocketAddr = SocketAddr{ ip: Ipv4Addr(192, 168, 1, 102), port: 2000 };

fn main() {
    println!("CHECKING UPNP SERVICES/DEVICES");
    check();
    
    println!("TRYING TO SETUP A PORT FORWARD ON LOCAL ROUTER");
    forward_port();
}

fn check() {
    let upnp = UPnPIntf::find_all(SocketAddr{ ip: Ipv4Addr(192, 168, 1, 102), port: 2000 }).unwrap();
    
    for i in upnp.iter() {
        println!("{} {}\n", i.usn(), i.location());
    }
}

fn forward_port() {
    let upnp = UPnPIntf::find_services(LOCAL_ADDR, 
        "WANIPConnection", "1")
    .unwrap();
    let service = upnp[0].service_desc().unwrap();
    
    service.send_action("AddPortMapping", &[("NewRemoteHost", ""),
        ("NewExternalPort", "6882"),
        ("NewProtocol", "TCP"),
        ("NewInternalPort", "6882"),
        ("NewInternalClient", LOCAL_ADDR.ip.to_string().as_slice()),
        ("NewEnabled", "1"),
        ("NewPortMappingDescription", "bittorrent-rs"),
        ("NewLeaseDuration", "0")])
    .unwrap();
}