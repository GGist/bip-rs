use bip_util::bt::{self};
use bip_utracker::{TrackerClient, ClientRequest, ClientError};
use bip_utracker::announce::{ClientState, AnnounceEvent};
use futures::stream::Stream;
use futures::future::Either;

use {handshaker};

#[test]
#[allow(unused)]
fn positive_client_request_failed() {
    let (sink, stream) = handshaker();
    
    let server_addr = "127.0.0.1:3503".parse().unwrap();
    // Dont actually create the server :) since we want the request to wait for a little bit until we drop
    
    let send_token = {
        let mut client = TrackerClient::new("127.0.0.1:4503".parse().unwrap(), sink).unwrap();
        
        let send_token = client.request(server_addr, ClientRequest::Announce(
            [0u8; bt::INFO_HASH_LEN].into(),
            ClientState::new(0, 0, 0, AnnounceEvent::None)
        )).unwrap();
        
        send_token
    };
    // Client is now dropped

    let mut blocking_stream = stream.wait();

    let metadata = match blocking_stream.next().unwrap().unwrap() {
        Either::B(b) => b,
        Either::A(_) => unreachable!()   
    };

    assert_eq!(send_token, metadata.token());
    
    match metadata.result() {
        &Err(ClientError::ClientShutdown) => (),
        _ => panic!("Did Not Receive ClientShutdown...")
    }
}