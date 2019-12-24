use std::thread::{self};
use std::time::{Duration};
use std::net::SocketAddr;

use bip_handshake::{Protocol};
use bip_util::bt::{self};
use bip_utracker::{TrackerClient, TrackerServer, ClientRequest};
use bip_utracker::announce::{ClientState, AnnounceEvent};
use futures::stream::Stream;
use futures::future::Either;

use crate::{handshaker, MockTrackerHandler};

#[test]
#[allow(unused)]
fn positive_announce_started() {
    let (sink, stream) = handshaker();
    
    let server_addr = "127.0.0.1:3501".parse().unwrap();
    let mock_handler = MockTrackerHandler::new();
    let server = TrackerServer::run(server_addr, mock_handler).unwrap();
    
    thread::sleep(Duration::from_millis(100));
    
    let mut client = TrackerClient::new("127.0.0.1:4501".parse().unwrap(), sink).unwrap();
    
    let hash = [0u8; bt::INFO_HASH_LEN].into();
    let send_token = client.request(server_addr, ClientRequest::Announce(
        hash,
        ClientState::new(0, 0, 0, AnnounceEvent::Started)
    )).unwrap();
    
    let mut blocking_stream = stream.wait();

    let init_msg = match blocking_stream.next().unwrap().unwrap() {
        Either::A(a) => a,
        Either::B(_) => unreachable!()
    };

    let exp_peer_addr: SocketAddr = "127.0.0.1:6969".parse().unwrap();

    assert_eq!(&Protocol::BitTorrent, init_msg.protocol());
    assert_eq!(&exp_peer_addr, init_msg.address());
    assert_eq!(&hash, init_msg.hash());

    let metadata = match blocking_stream.next().unwrap().unwrap() {
        Either::B(b) => b,
        Either::A(_) => unreachable!()   
    };
    let metadata_result = metadata.result().as_ref().unwrap().announce_response().unwrap();

    assert_eq!(metadata_result.leechers(), 1);
    assert_eq!(metadata_result.seeders(), 1);
    assert_eq!(metadata_result.peers().iter().count(), 1);
}