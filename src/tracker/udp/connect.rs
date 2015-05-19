//! Acquiring A Connection ID From Tracker.

use std::rand;
use std::time::duration::{Duration};
use std::old_io::{IoResult, ConnectionFailed, Writer, BufWriter, BufReader};

use util;
use types::{Timepoint};
use super::stream::{UdpStream};

const SEND_MESSAGE_LEN: usize = 16;
const RECV_MESSAGE_LEN: usize = 16;

const PROTOCOL_ID: i64 = 0x41727101980;

const CONNECT_MESSAGE_ID: i32 = 0;
const ID_EXPIRE_SECONDS:  i64 = 60;

/// Wraps a connection id and expiration timer for UDP Tracker requests.
pub struct ConnectID {
    id:     i64,
    expire: Timepoint
}

impl ConnectID {
    /// Sends a request for a UDP Tracker connection id to the remote host.
    ///
    /// This is a blocking operation.
    pub fn request(conn: &mut UdpStream) -> IoResult<ConnectID> {
        let mut send_buf = [0u8; SEND_MESSAGE_LEN];
        let mut recv_buf = [0u8; RECV_MESSAGE_LEN];
        
        let trans_id = try!(write_connect_request(&mut send_buf));
        try!(conn.request(&send_buf, &mut recv_buf, super::TRACKER_MAX_ATTEMPTS, super::udp_tracker_wait));
        
        let id = try!(read_connect_response(&recv_buf, trans_id));
        let expire = try!(Timepoint::new(Duration::seconds(ID_EXPIRE_SECONDS)));
        
        Ok(ConnectID{ id: id, expire: expire })
    }
    
    ///Checks if the connection id is still valid and optionally returns it.
    pub fn connect_id(&self) -> Option<i64> {
        if self.is_expired() {
            None
        } else {
            Some(self.id)
        }
    }
    
    /// Returns true if the connection id has expired.
    pub fn is_expired(&self) -> bool {
        self.expire.has_passed()
    }
}

/// Writes a connection request into the buffer.
/// 
/// Returns the transaction id that should be matched up in the response.
fn write_connect_request(buf: &mut [u8]) -> IoResult<i32> {
    let mut buf_writer = BufWriter::new(buf);
    
    try!(buf_writer.write_be_i64(PROTOCOL_ID));
    try!(buf_writer.write_be_i32(CONNECT_MESSAGE_ID));
    
    let trans_id = rand::random::<super::TransID>();
    try!(buf_writer.write_be_i32(trans_id));
    
    Ok(trans_id)
}

/// Reads a connection response from the buffer.
/// 
/// Returns the connection id.
fn read_connect_response(buf: &[u8], trans_id: i32) -> IoResult<i64> {
    let mut buf_reader = BufReader::new(buf);
    
    if try!(buf_reader.read_be_i32()) != CONNECT_MESSAGE_ID {
        return Err(util::simple_ioerror(ConnectionFailed, "Tracker Responded To Different Action (Not Connect)"))
    }
    
    if try!(buf_reader.read_be_i32()) != trans_id {
        return Err(util::simple_ioerror(ConnectionFailed, "Tracker Responded With Different Transaction ID"))
    }
    
    buf_reader.read_be_i64()
}