use std::thread::{self};
use std::time::Duration;

use bip_util::bt::{self};
use bip_utracker::{ClientRequest, TrackerClient, TrackerServer};
use futures::stream::Stream;

use crate::{handshaker, MockTrackerHandler};

#[test]
#[allow(unused)]
fn positive_connection_id_cache() {
    let (sink, mut stream) = handshaker();

    let server_addr = "127.0.0.1:3506".parse().unwrap();
    let mock_handler = MockTrackerHandler::new();
    let server = TrackerServer::run(server_addr, mock_handler.clone()).unwrap();

    thread::sleep(Duration::from_millis(100));

    let mut client = TrackerClient::new("127.0.0.1:4506".parse().unwrap(), sink).unwrap();

    let first_hash = [0u8; bt::INFO_HASH_LEN].into();
    let second_hash = [1u8; bt::INFO_HASH_LEN].into();

    let mut blocking_stream = stream.wait();

    client
        .request(server_addr, ClientRequest::Scrape(first_hash))
        .unwrap();
    blocking_stream.next().unwrap();

    assert_eq!(mock_handler.num_active_connect_ids(), 1);

    for _ in 0..10 {
        client
            .request(server_addr, ClientRequest::Scrape(second_hash))
            .unwrap();
    }

    for _ in 0..10 {
        blocking_stream.next().unwrap();
    }

    assert_eq!(mock_handler.num_active_connect_ids(), 1);
}
