use bip_handshake::{HandshakerBuilder, InitiateMessage, Protocol, DiscoveryInfo};
use bip_handshake::transports::TcpTransport;

use bip_util::bt::{self};
use tokio_core::reactor::{Core};
use futures::Future;
use futures::stream::Stream;
use futures::sink::Sink;

#[test]
fn positive_connect() {
    let mut core = Core::new().unwrap();

    let mut handshaker_one_addr = "127.0.0.1:0".parse().unwrap();
    let handshaker_one_pid = [4u8; bt::PEER_ID_LEN].into();

    let handshaker_one = HandshakerBuilder::new()
        .with_bind_addr(handshaker_one_addr)
        .with_peer_id(handshaker_one_pid)
        .build(TcpTransport, core.handle()).unwrap();

    handshaker_one_addr.set_port(handshaker_one.port());

    let mut handshaker_two_addr = "127.0.0.1:0".parse().unwrap();
    let handshaker_two_pid = [5u8; bt::PEER_ID_LEN].into();

    let handshaker_two = HandshakerBuilder::new()
        .with_bind_addr(handshaker_two_addr)
        .with_peer_id(handshaker_two_pid)
        .build(TcpTransport, core.handle()).unwrap();

    handshaker_two_addr.set_port(handshaker_two.port());

    let (item_one, item_two) = core.run(handshaker_one
        .send(InitiateMessage::new(Protocol::BitTorrent, [55u8; bt::INFO_HASH_LEN].into(), handshaker_two_addr))
        .map_err(|_| ())
        .and_then(|handshaker_one| {
            handshaker_one.into_future()
                .join(handshaker_two.into_future())
                .map_err(|_| ())
        })
        .map(|((opt_item_one, _), (opt_item_two, _))| {
            (opt_item_one.unwrap(), opt_item_two.unwrap())
        })
    ).unwrap();

    // Result from handshaker one should match handshaker two's listen address
    assert_eq!(handshaker_two_addr, *item_one.address());

    assert_eq!(handshaker_one_pid, *item_two.peer_id());
    assert_eq!(handshaker_two_pid, *item_one.peer_id());
}