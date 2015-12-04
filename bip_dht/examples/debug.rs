extern crate bip_dht;
extern crate bip_handshake;
extern crate bip_util;
extern crate log;

use std::collections::{HashSet};
use std::io::{self, Read};
use std::net::{ToSocketAddrs, SocketAddr};
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
    filter: HashSet<SocketAddr>
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
		0
	}

    /// Initiates a handshake with the given socket address.
    fn connect(&mut self, expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr) {
        if self.filter.contains(&addr) {
            return
        }
        
        self.filter.insert(addr);
		println!("Received peer {:?}, total {}", addr, self.filter.len());
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

fn main() {/*
	log::set_logger(|m| {
		m.set(LogLevelFilter::max());
		Box::new(SimpleLogger)
	}).unwrap();*/
	
    	let input = io::stdin();
	
	let hash = [0x55, 0xEE, 0x42, 0xA6, 0x25, 0xF9, 0xD5, 0x42, 0xA3, 0x7C, 0x6E, 0xC2, 0xA8, 0x5E, 0x9D, 0x2E, 0xBA, 0xA1, 0xF1, 0x9E];
    
    let address = ("0.0.0.0", 6889).to_socket_addrs().unwrap().next().unwrap();
        let dht = DhtBuilder::with_router(Router::Transmission).set_read_only(false)
        .set_source_addr(address).start_mainline(SimpleHandshaker{ filter: HashSet::new() }).unwrap();
        
        
        for event in dht.events().iter().take(2) {
            println!("Received dht event {:?}", event);
            dht.search(hash.into(), false);
        }
}