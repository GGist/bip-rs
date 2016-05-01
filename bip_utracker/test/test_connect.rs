use std::thread::{self};
use std::time::{Duration};
use std::sync::mpsc::{self};

use bip_util::bt::{self};
use bip_utracker::{TrackerClient, TrackerServer, ClientRequest};
use bip_utracker::announce::{ClientState, AnnounceEvent};

use {MockTrackerHandler, MockHandshaker};

#[test]
#[allow(unused)]
fn positive_receive_connect_id() {
    let (send, recv) = mpsc::channel();
    
    let server_addr = "127.0.0.1:3505".parse().unwrap();
    let mock_handler = MockTrackerHandler::new();
    let server = TrackerServer::run(server_addr, mock_handler).unwrap();
    
    thread::sleep(Duration::from_millis(100));
    
    let mut client = TrackerClient::new("127.0.0.1:4505".parse().unwrap(), MockHandshaker::new(send)).unwrap();
    
    let send_token = client.request(server_addr, ClientRequest::Announce(
        [0u8; bt::INFO_HASH_LEN].into(),
        ClientState::new(0, 0, 0, AnnounceEvent::None)
    )).unwrap();
    
    let metadata = recv.recv().unwrap();
    
    assert_eq!(send_token, metadata.token());
    assert!(metadata.result().is_ok());
}