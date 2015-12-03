extern crate bip_dht;
extern crate log;

use std::io::{self};
use std::net::{ToSocketAddrs};
use std::thread::{self};

use bip_dht::{DhtBuilder, Router, MainlineDht};

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

fn main() {
	log::set_logger(|m| {
		m.set(LogLevelFilter::max());
		Box::new(SimpleLogger)
	}).unwrap();
	
	let builder = DhtBuilder::with_router(Router::uTorrent)
		.set_source_addr(("0.0.0.0", 6881).to_socket_addrs().unwrap().next().unwrap());
	let dht = MainlineDht::with_builder(builder).unwrap();
	
	let mut hash = [0u8; 20];
	let mut input = String::new();
	let io = io::stdin();
	
	loop {
		io.read_line(&mut input).unwrap();
		
		let mut hex_byte = 0;
		for (index, &byte) in input.as_bytes().iter().enumerate() {
			let hex_value = if byte >= 48 && byte <= 57 {
				byte - 48
			} else if byte >= 65 && byte <= 70 {
				byte - 65
			} else {
				break;
			};
			
			if (index + 1) % 2 != 0 {
				hex_byte = hex_value;
				hex_byte <<= 4;
			} else {
				hex_byte |= hex_value;
				hash[index / 2] = hex_byte;
			}
		}
		
		hash = [0x34, 0x9C, 0xAD, 0xDE, 0x73, 0xEC, 0xF2, 0x7C, 0x10, 0xFC, 0x0B, 0x39, 0x5F, 0xE2, 0x94, 0x06, 0xB3, 0xAC, 0x09, 0xFC];
		
		dht.search(hash.into()).unwrap();
		
		input.clear();
	}
}