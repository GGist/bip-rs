use bip_util::bt::{self};
use bip_utracker::{TrackerClient, TrackerServer, ClientRequest};

use {MockTrackerHandler, MockHandshaker};

#[test]
#[allow(unused)]
fn positive_scrape() {
    let server_addr = "127.0.0.1:3506".parse().unwrap();
    let mock_handler = MockTrackerHandler::new();
    let server = TrackerServer::run(server_addr, mock_handler).unwrap();
    
    let mock_handshaker = MockHandshaker::new();
    let mut client = TrackerClient::new("127.0.0.1:4506".parse().unwrap(), mock_handshaker.clone()).unwrap();
    let responses = client.responses();
    
    let send_token = client.request(server_addr, ClientRequest::Scrape([0u8; bt::INFO_HASH_LEN].into())).unwrap();
    
    let (recv_token, res) = responses.recv().unwrap();
    
    assert_eq!(send_token, recv_token);
    
    let response = res.as_ref().unwrap().scrape_response().unwrap();
    assert_eq!(response.iter().count(), 1);
    
    let stats = response.iter().next().unwrap();
    assert_eq!(stats.num_seeders(), 0);
    assert_eq!(stats.num_downloads(), 0);
    assert_eq!(stats.num_leechers(), 0);
    
    mock_handshaker.connects_received(|connects| {
        assert_eq!(connects.len(), 0);
    });
}