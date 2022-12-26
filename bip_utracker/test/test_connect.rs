use std::thread::{self};
use std::time::{Duration};

use bip_util::bt::{self};
use bip_utracker::{TrackerClient, TrackerServer, ClientRequest};
use bip_utracker::announce::{ClientState, AnnounceEvent};
use futures::stream::Stream;
use futures::future::Either;

use crate::{handshaker, MockTrackerHandler};

#[test]
#[allow(unused)]
fn positive_receive_connect_id() {
    let (sink, stream) = handshaker();
    
    let server_addr = "127.0.0.1:3505".parse().unwrap();
    let mock_handler = MockTrackerHandler::new();
    let server = TrackerServer::run(server_addr, mock_handler).unwrap();
    
    thread::sleep(Duration::from_millis(100));
    
    let mut client = TrackerClient::new("127.0.0.1:4505".parse().unwrap(), sink).unwrap();
    
    let send_token = client.request(server_addr, ClientRequest::Announce(
        [0u8; bt::INFO_HASH_LEN].into(),
        ClientState::new(0, 0, 0, AnnounceEvent::None)
    )).unwrap();
    
    let mut blocking_stream = stream.wait();

    let _init_msg = match blocking_stream.next().unwrap().unwrap() {
        Either::A(a) => a,
        Either::B(_) => unreachable!()
    };

    let metadata = match blocking_stream.next().unwrap().unwrap() {
        Either::B(b) => b,
        Either::A(_) => unreachable!()   
    };
    
    assert_eq!(send_token, metadata.token());
    assert!(metadata.result().is_ok());
}