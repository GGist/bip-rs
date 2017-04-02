use std::mem;

use bip_util::bt::{self};
use bip_utracker::{TrackerClient, ClientRequest};
use bip_utracker::announce::{ClientState, AnnounceEvent};
use futures::stream::Stream;
use futures::{Future};

use {handshaker};

#[test]
#[allow(unused)]
fn positive_client_request_dropped() {
    let (sink, mut stream) = handshaker();
    
    let server_addr = "127.0.0.1:3504".parse().unwrap();
    
    let request_capacity = 10;
    
    let mut client = TrackerClient::with_capacity("127.0.0.1:4504".parse().unwrap(), sink, request_capacity).unwrap();
    
    for _ in 0..request_capacity {
        client.request(server_addr, ClientRequest::Announce(
            [0u8; bt::INFO_HASH_LEN].into(),
            ClientState::new(0, 0, 0, AnnounceEvent::Started)
        )).unwrap();
    }
    
    assert!(client.request(server_addr, ClientRequest::Announce(
            [0u8; bt::INFO_HASH_LEN].into(),
            ClientState::new(0, 0, 0, AnnounceEvent::Started)
    )).is_none());
    
    mem::drop(client);
    
    let buffer = stream.collect().wait().unwrap();
    assert_eq!(request_capacity, buffer.len());
}