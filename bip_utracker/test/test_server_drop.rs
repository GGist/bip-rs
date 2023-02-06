use std::net::UdpSocket;
use std::time::Duration;

use bip_utracker::request::{self, RequestType, TrackerRequest};
use bip_utracker::TrackerServer;

use crate::MockTrackerHandler;

#[test]
#[allow(unused)]
fn positive_server_dropped() {
    let server_addr = "127.0.0.1:3508".parse().unwrap();
    let mock_handler = MockTrackerHandler::new();

    {
        let server = TrackerServer::run(server_addr, mock_handler).unwrap();
    }
    // Server is now shut down

    let mut send_message = Vec::new();

    let request = TrackerRequest::new(request::CONNECT_ID_PROTOCOL_ID, 0, RequestType::Connect);
    request.write_bytes(&mut send_message).unwrap();

    let socket = UdpSocket::bind("127.0.0.1:4508").unwrap();
    socket.send_to(&send_message, server_addr);

    let mut receive_message = vec![0u8; 1500];
    socket.set_read_timeout(Some(Duration::from_millis(200)));
    let recv_result = socket.recv_from(&mut receive_message);

    assert!(recv_result.is_err());
}
