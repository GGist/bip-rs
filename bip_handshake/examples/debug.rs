extern crate bip_handshake;

use std::sync::mpsc::{self, Sender, Receiver};

use bip_handshake::{Handshaker, BTHandshaker, BTPeer};

fn main() {
    let (send, recv): (Sender<BTPeer>, Receiver<BTPeer>) = mpsc::channel();
    
    let handshaker = BTHandshaker::new(send, "10.0.0.18:12234".parse().unwrap(), [0u8; 20].into()).unwrap();
    handshaker.register_hash([0xf5, 0xb8, 0xca, 0x8f, 0x37, 0x11, 0x13, 0xe9, 0x33, 0x22, 0x86, 0x60, 0x22, 0x06, 0x22, 0xeb, 0x38, 0x2c, 0x8e, 0x06].into());
    
    println!("{:?}", handshaker.port());
    
    for peer in recv {
        println!("{:?}", peer);
    }
}