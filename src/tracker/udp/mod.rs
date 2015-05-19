//! Facilitates interaction with a UDP Tracker.

// NOTE: Technically this module should be operating at the packet level to send
// and receive responses from the UDP Tracker. However, we are striving to be
// dependency free at the moment and care has been taking to make sure that we
// operate our UDP connection in such a way so that MTUs are not exceeded and
// all dynamically sized responses sent to us are fully read from our side of
// the connection.

use std::{rand};
use std::num::{Int};
use std::thread::{Thread};
use std::time::duration::{Duration};
use std::sync::mpsc::{self};
use std::old_io::net::ip::{SocketAddr, Ipv4Addr, IpAddr, ToSocketAddr, Port};
use std::old_io::{IoResult, IoError, BufWriter, BufReader, ConnectionFailed, OtherIoError};

use self::connect::{ConnectID};
use self::stream::{UdpStream};
use util;
use types::{PeerID, InfoHash, Timepoint};
use tracker::{AnnounceInfo, ScrapeInfo, Tracker, TransferStatus};

mod connect;
mod stream;

pub type EventID = i32;
pub type TransID = i32;

// Action Types
const CONNECT_ACTION:  i32 = 0;
const ANNOUNCE_ACTION: i32 = 1;
const SCRAPE_ACTION:   i32 = 2;
const ERROR_ACTION:   i32 = 3;

// Event Types
const NONE_ID:      i32 = 0;
const COMPLETED_ID: i32 = 1;
const STARTED_ID:   i32 = 2;
const STOPPED_ID:   i32 = 3;

// Port Ranges To Use
const TRACKER_BASE_PORT:  Port = 6969;
const TRACKER_PORT_RANGE: u32  = 10;

// Number Of Bytes In Peer IP + Port
const BYTES_PER_PEER: usize = 4 + 2; // IP: 4, Port: 2

// Static Buffer Lengths
const ANNOUNCE_REQUEST_LEN:      usize = 100;
const ANNOUNCE_MIN_RESPONSE_LEN: usize = 20;
const ANNOUNCE_VAR_RESPONSE_LEN: usize = BYTES_PER_PEER * 50; // Peers: 50
const SCRAPE_REQUEST_LEN:        usize = 36;
const SCRAPE_RESPONSE_LEN:       usize = 20;
const ERROR_VAR_RESPONSE_LEN:    usize = 100; // Arbitrary Amount Of Bytes

// Part Of The Standard
pub const TRACKER_MAX_ATTEMPTS: u32 = 8;

// UDP Tracker Protocol Is Packet Based, So Reads Shouldn't Hang
const ASYNC_TIMEOUT_MILLI: u64 = 1;

/// An object facilitating communication with a tracker using the UDP Tracker Protocol.
pub struct UdpTracker {
    conn:         UdpStream,
    conn_id:      ConnectID,
    info_hash:    InfoHash,
    local_peer:   PeerID,
    port_forward: Port
}

impl UdpTracker {
    /// Attempts to create a connection with a UdpTracker on a local IPv4 interface.
    /// Remote peers will connect to your external IP Address using the port 
    /// passed in. This port should be forwarded on your NAT router.
    ///
    /// This is a blocking operation.
    pub fn connect<T: ToSocketAddr>(dst: T, info: InfoHash, peer: PeerID, forward: Port) 
        -> IoResult<UdpTracker> {
        let remote_addr = try!(dst.to_socket_addr());
        
        // Find An IPv4 Interface To Connect With The Tracker On
        let (stream, id) = try!(find_local_interface(remote_addr, TRACKER_BASE_PORT, 
            TRACKER_PORT_RANGE
        ));
        
        Ok(UdpTracker{ conn: stream, conn_id: id, info_hash: info, 
            local_peer: peer, port_forward: forward }
        )
    }
    
    /// Checks if the connection id is still valid and if not, creates a new one.
    ///
    /// Returns a valid connection id.
    fn check_connect_id(&mut self) -> IoResult<i64> {
        match self.conn_id.connect_id() {
            Some(id) => Ok(id),
            None     => {
                self.conn_id = try!(ConnectID::request(&mut self.conn));
                self.conn_id.connect_id().ok_or(
                    util::simple_ioerror(OtherIoError, "Connection ID Expired Too Fast")
                )
            }
        }
    }
    
    /// Make a scrape request to the tracker.
    ///
    /// Returns a ScrapeInfo object.
    fn scrape(&mut self) -> IoResult<ScrapeInfo> {
        let mut send_buffer = [0u8; SCRAPE_REQUEST_LEN];
        let 
        
        // Write Scrape Request
        let sent_id = try!(self.write_scrape(&mut send_buffer));
        
        // Make Scrape Request
        try!(make_request(&mut self.conn, &send_buffer[], sent_id, SCRAPE_ACTION));
        
        //Extract Scrape Response
        recv_scrape(&mut self.conn)
    }
    
    /// Writes a scrape request to the buffer.
    ///
    /// Returns the transaction id that should be matched in the response.
    fn write_scrape(&mut self, buf: &mut [u8]) -> IoResult<TransID> {
        let connect_id = try!(self.check_connect_id());
        let trans_id = rand::random::<TransID>();
        
        let mut buf_writer = BufWriter::new(buf);
        
        try!(buf_writer.write_be_i64(connect_id));
        try!(buf_writer.write_be_i32(SCRAPE_ACTION));
        try!(buf_writer.write_be_i32(trans_id));
        try!(buf_writer.write_all(self.info_hash.as_slice()));
        
        Ok(trans_id)
    }
    
    /// Make a parametrized announce request to the tracker.
    ///
    /// Returns an AnnounceInfo object.
    fn announce(&mut self, status: TransferStatus, event: EventID) -> IoResult<AnnounceInfo> {
        let mut send_buffer = [0u8; ANNOUNCE_REQUEST_LEN];
        
        // Write Announce Request
        let sent_id = try!(self.write_announce(&mut send_buffer, status, event));
        
        // Make Announce Request
        try!(make_request(&mut self.conn, &send_buffer[], sent_id, ANNOUNCE_ACTION));
        
        // Extract Announce Response
        recv_announce(&mut self.conn)
    }
    
    /// Writes a parametrized announce request to the buffer.
    ///
    /// Returns the transaction id that should be matched in the response.
    fn write_announce(&mut self, buf: &mut [u8], status: TransferStatus, event: EventID) -> IoResult<TransID> {
        let connect_id = try!(self.check_connect_id());
        let trans_id = rand::random::<TransID>();
        let key = rand::random::<u32>();
    
        let mut buf_writer = BufWriter::new(buf);
            
        try!(buf_writer.write_be_i64(connect_id));              // Connection ID
        try!(buf_writer.write_be_i32(ANNOUNCE_ACTION));         // Announce Request
        try!(buf_writer.write_be_i32(trans_id));                // Verify In Response
        try!(buf_writer.write_all(self.info_hash.as_slice()));  // Identify Torrent File
        try!(buf_writer.write_str(self.local_peer.as_slice())); // Identify Ourselves
        try!(buf_writer.write_be_i64(status.downloaded));       // Bytes Downloaded So Far
        try!(buf_writer.write_be_i64(status.remaining));        // Bytes Left To Download
        try!(buf_writer.write_be_i64(status.uploaded));         // Bytes Uploaded So Far
        try!(buf_writer.write_be_i32(event));                   // Event Type
        try!(buf_writer.write_be_u32(0));                       // IPv4 Address For Peers To Connect
        try!(buf_writer.write_be_u32(key));                     // Key (Helps With Endianness For Tracker?)
        try!(buf_writer.write_be_i32(-1));                      // Number Of Clients To Return (-1 Default)
        try!(buf_writer.write_be_u16(self.port_forward));       // Port For Peers To Connect
        try!(buf_writer.write_be_u16(0));                       // Extensions Bitmask
        
        Ok(trans_id)
    }
}

fn make_request(stream: &mut UdpStream, msg: &[u8], sent_id: TransID, sent_action: i32) -> IoResult<()> {
    let mut verify_buffer = [0u8; VERIFY_RESPONSE_LEN];
    
    let bytes_read = try!(stream.request(msg, &mut verify_buffer, TRACKER_MAX_ATTEMPTS, udp_tracker_wait));
    if bytes_read != verify_buffer.len() {
        return Err(util::simple_ioerror(OtherIoError, "UDP Tracker Sent An Incomplete Response"))
    }
    let mut buf_reader = BufReader::new(&verify_buffer[]);
    
    let recv_action = try!(buf_reader.read_be_i32());
    let recv_trans_id = try!(buf_reader.read_be_i32());
    
    if recv_trans_id != sent_id {
        return Err(util::simple_ioerror(OtherIoError, "Tracker Responded With A Different Transaction ID"))
    } else if recv_action == ERROR_ACTION {
        return Err(recv_error_message(stream))
    } else if recv_action != sent_action {
        return Err(wrong_event_received(sent_action, recv_action))
    }
    
    Ok(())
}

fn recv_scrape(stream: &mut UdpStream) -> IoResult<ScrapeInfo> {
    let mut recv_buffer = [0u8; SCRAPE_PAYLOAD_LEN];
    
    let bytes_read = try!(stream.recv(&mut recv_buffer[], Some(ASYNC_TIMEOUT_MILLI)));
    if bytes_read != recv_buffer.len() {
        return Err(util::simple_ioerror(OtherIoError, "UDP Tracker Sent An Incomplete Scrape Response"))
    }
    let mut buf_reader = BufReader::new(&recv_buffer[]);
    
    let seeders = try!(buf_reader.read_be_i32());
    let downloads = try!(buf_reader.read_be_i32());
    let leechers = try!(buf_reader.read_be_i32());
    
    Ok(ScrapeInfo{ leechers: leechers, seeders: seeders, downloads: downloads })
}

fn recv_announce(stream: &mut UdpStream) -> IoResult<AnnounceInfo> {
    let mut recv_buffer = [0u8; ANNOUNCE_MIN_PAYLOAD_LEN];
    
    let bytes_read = try!(stream.recv(&mut recv_buffer[], Some(ASYNC_TIMEOUT_MILLI)));
    if bytes_read != recv_buffer.len() {
        return Err(util::simple_ioerror(OtherIoError, "UDP Tracker Sent An Incomplete Announce Response"))
    }
    let mut buf_reader = BufReader::new(&recv_buffer[]);
    
    let interval = try!(buf_reader.read_be_i32());
    let interval_tp = try!(Timepoint::new(Duration::seconds(interval as i64)));
    
    let leechers = try!(buf_reader.read_be_i32());
    let seeders = try!(buf_reader.read_be_i32());
    let peers = try!(recv_peers(stream));
    
    Ok(AnnounceInfo{ leechers: leechers, seeders: seeders, peers: peers, interval: interval_tp })
}

/// Reads peer information on the given stream. Expects the peer information to
/// have already been received and therefore does not wait on a response if it
/// has not been sent to the stream.
fn recv_peers(stream: &mut UdpStream) -> IoResult<Vec<SocketAddr>> {
    let mut peers_buffer = [0u8; ANNOUNCE_VAR_PAYLOAD_LEN];
    
    let mut peer_list = Vec::new();
    while let Ok(bytes_read) = stream.recv(&mut peers_buffer, Some(ASYNC_TIMEOUT_MILLI)) {
        let mut buf_reader = BufReader::new(&peers_buffer[]);
        
        for _ in (0..bytes_read / BYTES_PER_PEER) {
            let ip = Ipv4Addr(try!(buf_reader.read_u8()), 
                try!(buf_reader.read_u8()),
                try!(buf_reader.read_u8()), 
                try!(buf_reader.read_u8())
            );
            
            let port = try!(buf_reader.read_be_u16());
            
            peer_list.push(try!((ip, port).to_socket_addr()));
        }
    }
        
    Ok(peer_list)
}

/// Reads an error message from the given UdpStream. Expects the error message
/// to have already been received and therefore does not wait on a response if
/// it has not been sent to the stream.
fn recv_error_message(stream: &mut UdpStream) -> IoError {
    let mut msg_buf = Vec::with_capacity(ERROR_VAR_RESPONSE_LEN);
    let mut pos = 0;
    
    while let Ok(bytes_read) = stream.recv(&mut msg_buf[pos..], Some(ASYNC_TIMEOUT_MILLI)) {
        if bytes_read == msg_buf.len() {
            let new_len = msg_buf.len() * 2;
            
            msg_buf.reserve(new_len);
        }
        
        pos += bytes_read;
    }
    
    match (String::from_utf8(msg_buf), pos != 0) {
        (Ok(msg), true) => IoError{ kind: OtherIoError, 
            desc: "Received Error Response For Scrape Request (See detail)",
            detail: Some(msg) },
        _ => IoError{ kind: OtherIoError,
            desc: "Received Error Response For Scrape Request (No detail)",
            detail: None }
    }
}

/// Standard wait algorithm defined in the UDP Tracker Protocol.
///
/// Returned waiting time is in milliseconds.
pub fn udp_tracker_wait(attempt: u64) -> u64 {
    15 * 2.pow(attempt as usize) * 1000
}

/// Returns an IoError with information regarding an expected event and a received event.
fn wrong_event_received(expected: EventID, received: EventID) -> IoError {
    match received {
        CONNECT_ACTION => IoError{
            kind: OtherIoError,
            desc: "UDP Tracker Sent Unexpected Connect Response (Check Detail)",
            detail: Some(format!("Expected EventID {} Received EventID {}", expected, received)) 
        },
        ANNOUNCE_ACTION => IoError{
            kind: OtherIoError,
            desc: "UDP Tracker Sent Unexpected Announce Response (Check Detail)",
            detail: Some(format!("Expected EventID {} Received EventID {}", expected, received)) 
        },
        SCRAPE_ACTION => IoError{
            kind: OtherIoError,
            desc: "UDP Tracker Sent Unexpected Scrape Response (Check Detail)",
            detail: Some(format!("Expected EventID {} Received EventID {}", expected, received)) 
        },
        _ => IoError{
            kind: OtherIoError,
            desc: "UDP Tracker Sent Unexpected Unknown Response (Check Detail)",
            detail: Some(format!("Expected EventID {} Received EventID {}", expected, received)) 
        }
    }
}

/// Finds the first ipv4 interface that responds to a connect request from the remote_addr.
///
/// Returns a UdpStream operating on that interface as well as an initial ConnectID.
fn find_local_interface(remote_addr: SocketAddr, local_port: Port, port_range: u32) -> IoResult<(UdpStream, ConnectID)> {
    let local_ip_addrs = try!(util::find_net_addrs());
    let (tx, rx) = mpsc::channel();

    { // Need to move tx in scope so that it gets destroyed before receiving
        let tx = tx;
        for ip in local_ip_addrs.into_iter() {
            let tx = tx.clone();
            
            Thread::spawn(move || {
                let udp_socket = util::open_udp(SocketAddr{ ip: ip, port: local_port }, port_range).unwrap();
                let mut udp_stream = UdpStream::new(udp_socket, remote_addr).unwrap();
                
                match ConnectID::request(&mut udp_stream) {
                    Ok(connect_id) => return tx.send((udp_stream, connect_id)).unwrap(),
                    Err(e)         => println!("{:?}", e)
                };
            });
        }
    }
        
    let (udp_stream, connect_id) = try!(rx.recv().map_err( |_|
        util::simple_ioerror(ConnectionFailed, "Could Not Communicate On Any IPv4 Interfaces")
    ));
    
    Ok((udp_stream, connect_id))
}

impl Tracker for UdpTracker {
    fn local_ip(&mut self) -> IpAddr {
        self.conn.local_sock().ip
    }

    fn send_scrape(&mut self) -> IoResult<ScrapeInfo> {
        self.scrape()
    }

    fn start_announce(&mut self, remaining: i64) -> IoResult<AnnounceInfo> {
        let status = TransferStatus{ downloaded: 0, remaining: remaining, uploaded: 0 };
    
        self.announce(status, STARTED_ID)
    }
    
    fn update_announce(&mut self, status: TransferStatus) -> IoResult<AnnounceInfo> {
        self.announce(status, NONE_ID)
    }

    fn stop_announce(&mut self, status: TransferStatus) -> IoResult<()> {
        let _ = try!(self.announce(status, STOPPED_ID));
        
        Ok(())
    }

    fn complete_announce(&mut self, status: TransferStatus) -> IoResult<AnnounceInfo> {
        let status = try!(self.announce(status, COMPLETED_ID));
        
        Ok(status)
    }
}