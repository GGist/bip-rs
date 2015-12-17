extern crate bip_bencode;
extern crate bip_dht;

use std::fs::{File};
use std::io::{Read};
use bip_bencode::{Bencode};
use bip_dht::message::{MessageType};
use bip_dht::message::response::{ExpectedResponse};

fn main() {
	let mut bytes = Vec::new();
	let mut file = File::open("C:/Users/GG/Desktop/request").unwrap();
	
	file.read_to_end(&mut bytes).unwrap();
	
	for i in bytes.iter() {
		print!("{} ", *i);
	}
	
	let bencode = Bencode::decode(&bytes).unwrap();
	
	println!("{:?}", bencode);
	let message = MessageType::new(&bencode, |_| ExpectedResponse::FindNode).unwrap();
	
	println!("{:?}", message);
}