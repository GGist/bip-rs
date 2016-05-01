use std::thread::{self};
use std::time::{Duration};
use std::sync::mpsc::{self};

use bip_util::bt::{self};
use bip_utracker::{TrackerClient, TrackerServer, ClientRequest};

use {MockTrackerHandler, MockHandshaker};

#[test]
#[allow(unused)]
fn positive_scrape() {
    let (send, recv) = mpsc::channel();
    
    let server_addr = "127.0.0.1:3507".parse().unwrap();
    let mock_handler = MockTrackerHandler::new();
    let server = TrackerServer::run(server_addr, mock_handler).unwrap();
    
    thread::sleep(Duration::from_millis(100));
    
    let mock_handshaker = MockHandshaker::new(send);
    let mut client = TrackerClient::new("127.0.0.1:4507".parse().unwrap(), mock_handshaker.clone()).unwrap();
    
    let send_token = client.request(server_addr, ClientRequest::Scrape([0u8; bt::INFO_HASH_LEN].into())).unwrap();
    
    let metadata = recv.recv().unwrap();
    
    assert_eq!(send_token, metadata.token());
    
    let response = metadata.result().as_ref().unwrap().scrape_response().unwrap();
    assert_eq!(response.iter().count(), 1);
    
    let stats = response.iter().next().unwrap();
    assert_eq!(stats.num_seeders(), 0);
    assert_eq!(stats.num_downloads(), 0);
    assert_eq!(stats.num_leechers(), 0);
    
    mock_handshaker.connects_received(|connects| {
        assert_eq!(connects.len(), 0);
    });
}