use std::thread;
use std::io::{Write, Read};
use std::net::TcpStream;

use bip_handshake::{HandshakerBuilder, DiscoveryInfo};
use bip_handshake::transports::TcpTransport;

use bip_util::bt::{self};
use tokio_core::reactor::{Core};
use tokio_io::io;
use futures::Future;
use futures::stream::Stream;

#[test]
fn positive_recover_bytes() {
    let mut core = Core::new().unwrap();

    let mut handshaker_one_addr = "127.0.0.1:0".parse().unwrap();
    let handshaker_one_pid = [4u8; bt::PEER_ID_LEN].into();

    let handshaker_one = HandshakerBuilder::new()
        .with_bind_addr(handshaker_one_addr)
        .with_peer_id(handshaker_one_pid)
        .build(TcpTransport, core.handle()).unwrap();

    handshaker_one_addr.set_port(handshaker_one.port());

    thread::spawn(move || {
        let mut stream = TcpStream::connect(handshaker_one_addr).unwrap();
        let mut write_buffer = Vec::new();

        write_buffer.write_all(&[1, 1]).unwrap();
        write_buffer.write_all(&[0u8; 8]).unwrap();
        write_buffer.write_all(&[0u8; bt::INFO_HASH_LEN]).unwrap();
        write_buffer.write_all(&[0u8; bt::PEER_ID_LEN]).unwrap();
        let expect_read_length = write_buffer.len();
        write_buffer.write_all(&[55u8; 100]).unwrap();

        stream.write_all(&write_buffer).unwrap();

        stream.read_exact(&mut vec![0u8; expect_read_length][..]).unwrap();
    });

    let recv_buffer = core.run(handshaker_one.into_future()
        .map_err(|_| ())
        .and_then(|(opt_message, _)| {
            let (_, _, _, _, _, sock) = opt_message.unwrap().into_parts();

            io::read_exact(sock, vec![0u8; 100])
                .map_err(|_| ())
        })
        .and_then(|(_, buf)| {
            Ok(buf)
        })).unwrap();

    // Assert that our buffer contains the bytes after the handshake
    assert_eq!(vec![55u8; 100], recv_buffer);
}