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
		.set_source_addr(("0.0.0.0", 6884).to_socket_addrs().unwrap().next().unwrap());
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
		
		hash = [0xA0, 0xAA, 0x05, 0x6F, 0xC8, 0x6E, 0xF8, 0x80, 0x5D, 0xFD, 0x55, 0xB2, 0x99, 0xC7, 0x84, 0x82, 0x1E, 0x7B, 0x14, 0x96];
		
		dht.search(hash.into()).unwrap();
		
		input.clear();
	}
}