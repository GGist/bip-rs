use std::thread::{self};
use std::time::{Duration};

use bip_util::bt::{self};
use bip_utracker::{TrackerClient, TrackerServer, ClientRequest};
use futures::stream::Stream;
use futures::future::Either;

use {handshaker, MockTrackerHandler};

#[test]
#[allow(unused)]
fn positive_scrape() {
    let (sink, stream) = handshaker();
    
    let server_addr = "127.0.0.1:3507".parse().unwrap();
    let mock_handler = MockTrackerHandler::new();
    let server = TrackerServer::run(server_addr, mock_handler).unwrap();
    
    thread::sleep(Duration::from_millis(100));
    
    let mut client = TrackerClient::new("127.0.0.1:4507".parse().unwrap(), sink).unwrap();
    
    let send_token = client.request(server_addr, ClientRequest::Scrape([0u8; bt::INFO_HASH_LEN].into())).unwrap();
    
    let mut blocking_stream = stream.wait();

    let metadata = match blocking_stream.next().unwrap().unwrap() {
        Either::B(b) => b,
        Either::A(_) => unreachable!()   
    };
    
    assert_eq!(send_token, metadata.token());
    
    let response = metadata.result().as_ref().unwrap().scrape_response().unwrap();
    assert_eq!(response.iter().count(), 1);
    
    let stats = response.iter().next().unwrap();
    assert_eq!(stats.num_seeders(), 0);
    assert_eq!(stats.num_downloads(), 0);
    assert_eq!(stats.num_leechers(), 0);
}