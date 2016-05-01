use std::thread::{self};
use std::time::{Duration};
use std::sync::mpsc::{self};

use bip_util::bt::{self};
use bip_utracker::{TrackerClient, TrackerServer, ClientRequest};
use bip_utracker::announce::{ClientState, AnnounceEvent};

use {MockTrackerHandler, MockHandshaker};

#[test]
#[allow(unused)]
fn positive_announce_stopped() {
    let (send, recv) = mpsc::channel();
    
    let server_addr = "127.0.0.1:3502".parse().unwrap();
    let mock_handler = MockTrackerHandler::new();
    let server = TrackerServer::run(server_addr, mock_handler).unwrap();
    
    thread::sleep(Duration::from_millis(100));
    
    let mock_handshaker = MockHandshaker::new(send);
    let mut client = TrackerClient::new("127.0.0.1:4502".parse().unwrap(), mock_handshaker.clone()).unwrap();
    
    let info_hash = [0u8; bt::INFO_HASH_LEN].into();
    
    // Started
    {
        let send_token = client.request(server_addr, ClientRequest::Announce(
            info_hash,
            ClientState::new(0, 0, 0, AnnounceEvent::Started)
        )).unwrap();
        
        let metadata = recv.recv().unwrap();
        
        assert_eq!(send_token, metadata.token());
        
        let response = metadata.result().as_ref().unwrap().announce_response().unwrap();
        assert_eq!(response.leechers(), 1);
        assert_eq!(response.seeders(), 1);
        assert_eq!(response.peers().iter().count(), 1);
        
        mock_handshaker.connects_received(|connects| {
            assert_eq!(connects.len(), 1);
        });
    }
    
    // Stopped
    {
        let send_token = client.request(server_addr, ClientRequest::Announce(
            info_hash,
            ClientState::new(0, 0, 0, AnnounceEvent::Stopped)
        )).unwrap();
        
        let metadata = recv.recv().unwrap();
        
        assert_eq!(send_token, metadata.token());
        
        let response = metadata.result().as_ref().unwrap().announce_response().unwrap();
        assert_eq!(response.leechers(), 0);
        assert_eq!(response.seeders(), 0);
        assert_eq!(response.peers().iter().count(), 0);
        
        mock_handshaker.connects_received(|connects| {
            assert_eq!(connects.len(), 1);
        });
    }
}