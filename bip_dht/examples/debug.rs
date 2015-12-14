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
		0
	}

    /// Initiates a handshake with the given socket address.
    fn connect(&mut self, expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr) {
        /*if self.filter.contains(&addr) {
            return
        }
        
        self.filter.insert(addr);*/
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
	
	let hash = [0x52, 0x47, 0x4E, 0x14, 0xC2, 0x58, 0x50, 0x53, 0x44, 0xF9, 0xB2, 0x6E, 0x80, 0x64, 0xA2, 0x0F, 0x47, 0x5F, 0x00, 0x70];
    
    let address = ("0.0.0.0", 6889).to_socket_addrs().unwrap().next().unwrap();
    let dht = DhtBuilder::with_router(Router::Transmission).set_read_only(false)
        .set_source_addr(address).start_mainline(SimpleHandshaker{ filter: HashSet::new(), count: 0 }).unwrap();
    
    for event in dht.events().iter().take(2) {
        
    }
    
    println!("DONE");
}