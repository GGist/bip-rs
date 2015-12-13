extern crate bip_dht;
extern crate bip_handshake;
extern crate bip_util;
extern crate log;

use std::collections::{HashSet};
use std::io::{self, Read};
use std::net::{ToSocketAddrs, SocketAddr, SocketAddrV4, Ipv4Addr};
use std::thread::{self};

use bip_dht::{DhtBuilder, Router, MainlineDht};
use bip_handshake::{Handshaker};
use bip_util::bt::{InfoHash, PeerId};

use log::{LogRecord, LogLevel, LogMetadata, LogLevelFilter};

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        metadata.level() <= LogLevel::Info
    }

    fn log(&self, record: &LogRecord) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }
}

struct SimpleHandshaker {
    filter: HashSet<SocketAddr>,
    count: usize
}

impl Handshaker for SimpleHandshaker {
    /// Type of stream used to receive connections from.
    type Stream = ();

    /// Unique peer id used to identify ourselves to other peers.
    fn id(&self) -> PeerId {
		[0u8; 20].into()
	}

    /// Advertise port that is being listened on by the handshaker.
    ///
    /// It is important that this is the external port that the peer will be sending data
    /// to. This is relevant if the client employs nat traversal via upnp or other means.
    fn port(&self) -> u16 {
		6820
	}

    /// Initiates a handshake with the given socket address.
    fn connect(&mut self, expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr) {
        let socket_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(104, 236, 141, 221), 6820));
        if addr == socket_addr {
            println!("FOUND OUR ADDRESS {:?}", addr);
        }
        
        if self.filter.contains(&addr) {
            return
        }
        
        self.filter.insert(addr);
        self.count += 1;
        println!("Received peer {:?}, total {}", addr, self.count);
	}
    
    /// Adds a filter that is applied to handshakes before they are initiated or completed.
    fn filter<F>(&mut self, process: Box<F>) where F: Fn(SocketAddr) -> bool + Send {
		()
	}
    
    /// Stream that connections for the specified hash are sent to after they are successful.
    ///
    /// Connections MAY be dropped if all streams for a given hash are destroyed.
    fn stream(&self, hash: InfoHash) -> () {
		()
	}
}

fn main() {
	log::set_logger(|m| {
		m.set(LogLevelFilter::max());
		Box::new(SimpleLogger)
	}).unwrap();
	
	let hash = [0x5e, 0x13, 0x6c, 0xff, 0x06, 0xd4, 0x5f, 0x9f, 0x5f, 0xff, 0x10, 0x7f,
        0x7a, 0xff, 0xfa, 0x55, 0x3a, 0xea, 0xd0, 0xcc];
    
    let address = ("0.0.0.0", 6889).to_socket_addrs().unwrap().next().unwrap();
    let dht = DhtBuilder::with_router(Router::Transmission).set_source_addr(address)
    .start_mainline(SimpleHandshaker{ filter: HashSet::new(), count: 0 }).unwrap();
    
    let stdin = io::stdin();
    
    let events = dht.events();
    thread::spawn(move || {
        for event in events {
            println!("RECEIVED DHT EVENT {:?}", event);
        }
    });
    
    let mut count = 0;
    let mut announce = false;
    for byte in stdin.bytes() {
        if count == 0 {
            count += 1;
        } else {
            dht.search(hash.into(), announce);
            announce = !announce;
            count = 0;
        }
    }
}