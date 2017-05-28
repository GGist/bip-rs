extern crate bip_handshake;
extern crate futures;
extern crate tokio_core;

use std::time::Duration;
use std::thread;
use std::io::{self, Write, BufRead};
use std::net::{SocketAddr, ToSocketAddrs};

use bip_handshake::{HandshakerBuilder, InitiateMessage, Protocol};
use bip_handshake::transports::TcpTransport;
use futures::{Future, Sink, Stream};
use tokio_core::reactor::Core;

fn main() {
    let mut stdout = io::stdout();
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    stdout.write(b"Enter An InfoHash In Hex Format: ").unwrap();
    stdout.flush().unwrap();

    let hex_hash = lines.next().unwrap().unwrap();
    let hash = hex_to_bytes(&hex_hash).into();

    stdout.write(b"Enter An Address And Port (eg: addr:port): ").unwrap();
    stdout.flush().unwrap();

    let str_addr = lines.next().unwrap().unwrap();
    let addr = str_to_addr(&str_addr);

    let mut core = Core::new().unwrap();

    // Show up as a uTorrent client...
    let peer_id = (*b"-UT2060-000000000000").into();
    let handshaker = HandshakerBuilder::new()
        .with_peer_id(peer_id)
        .build::<TcpTransport>(core.handle())
        .unwrap()
        .send(InitiateMessage::new(Protocol::BitTorrent, hash, addr))
        .wait()
        .unwrap();

    let _peer = core.run(
            handshaker.into_future().map(|(opt_peer, _)| opt_peer.unwrap())
        ).unwrap_or_else(|_| panic!(""));
    
    println!("\nConnection With Peer Established...Closing In 10 Seconds");

    thread::sleep(Duration::from_millis(10000));
}

fn hex_to_bytes(hex: &str) -> [u8; 20] {
    let mut exact_bytes = [0u8; 20];

    for byte_index in 0..20 {
        let high_index = byte_index * 2;
        let low_index = (byte_index * 2) + 1;

        let hex_chunk = &hex[high_index..low_index + 1];
        let byte_value = u8::from_str_radix(hex_chunk, 16).unwrap();

        exact_bytes[byte_index] = byte_value;
    }

    exact_bytes
}

fn str_to_addr(addr: &str) -> SocketAddr {
    addr.to_socket_addrs().unwrap().next().unwrap()
}