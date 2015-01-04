use std::io::{BufReader, BufWriter, BufferedStream, IoResult, IoError, Closed};
use std::default::{Default};
use std::io::net::tcp::{TcpStream};
use std::io::net::ip::{SocketAddr};

use peer::{Peer};
use util::{self, SPeerID, UPeerID, UInfoHash, UBTP10, Choice};

// TODO: Need To Add A One Time Check Somewhere That This Is Valid ASCII
const BTP_10_PROTOCOL: &'static str = "BitTorrent protocol";
const BTP_10_HANDSHAKE_LEN: uint = 68;

pub struct Handshake {
    conn: TcpStream,
    remote_id: SPeerID
}

impl Handshake {
    pub fn initiate(recipient: SocketAddr, info: &UInfoHash, curr_id: &UPeerID, valid: |&UPeerID| -> bool) 
        -> IoResult<Handshake> {
        let mut conn = try!(TcpStream::connect(recipient));
        let mut handshake = [0u8; BTP_10_HANDSHAKE_LEN];
        
        try!(Handshake::write_handshake(&mut handshake, BTP_10_PROTOCOL.as_bytes(), info, curr_id));
        try!(conn.write(&handshake));
        
        try!(conn.read_at_least(handshake.len(), &mut handshake));
        let peer_id = try!(Handshake::verify_handshake(&handshake, BTP_10_PROTOCOL.as_bytes(), info, valid));
        
        Ok(Handshake{ conn: conn,
            remote_id: peer_id }
        )
    }
    
    pub fn complete(mut initiater: TcpStream, info: &UInfoHash, curr_id: &UPeerID, valid: |&UPeerID| -> bool)
        -> IoResult<Handshake> {
        let mut handshake = [0u8; BTP_10_HANDSHAKE_LEN];
        
        try!(initiater.read_at_least(handshake.len(), &mut handshake));
        let peer_id = try!(Handshake::verify_handshake(&handshake, BTP_10_PROTOCOL.as_bytes(), info, valid));
        
        try!(Handshake::write_handshake(&mut handshake, BTP_10_PROTOCOL.as_bytes(), info, curr_id));
        try!(initiater.write(&handshake));
        
        Ok(Handshake{ conn: initiater,
            remote_id: peer_id }
        )
    }
    
    pub fn into_peer(self, num_pieces: u32) -> Peer {
        Peer{ conn_buf: BufferedStream::new(self.conn),
            self_state: Default::default(),
            remote_state: Default::default(),
            remote_id: self.remote_id,
            remote_pieces: Choice::Two(num_pieces) }
    }
    
    fn verify_handshake(bytes: &[u8], curr_prot: &UBTP10, info: &UInfoHash, valid: |&UPeerID| -> bool)
        -> IoResult<SPeerID> {
        let mut buf = BufReader::new(bytes);
        
        let remote_length = try!(buf.read_u8());
        if remote_length as uint != curr_prot.len() {
            return Err(IoError{ kind: Closed, desc: "Invalid Handshake Length", detail: None })
        }
        
        let mut remote_prot = [0u8; util::BTP_10_LEN];
        try!(buf.read_at_least(remote_prot.len(), &mut remote_prot));
        for (&rem, &cur) in remote_prot.iter().zip(curr_prot.iter()) {
            if rem != cur {
                return Err(IoError{ kind: Closed, desc: "Different Protocols In Use", detail: None })
            }
        }
        
        // Reserved Bytes
        try!(buf.read_be_u64());
        
        let mut remote_info = [0u8; util::INFO_HASH_LEN];
        try!(buf.read_at_least(remote_info.len(), &mut remote_info));
        for (rem, inf) in remote_info.iter().zip(info.iter()) {
            if rem != inf {
               return Err(IoError{ kind: Closed, desc: "Different Info Hash In Use", detail: None }) 
            }
        }
        
        let mut peer_id = [0u8; util::PEER_ID_LEN];
        try!(buf.read_at_least(util::PEER_ID_LEN, &mut peer_id));
        if !valid(peer_id.as_slice()) {
            return Err(IoError{ kind: Closed, desc: "Current Peer ID In Use", detail: None })
        }
        
        Ok(peer_id)
    }
    
    fn write_handshake(buf: &mut [u8], curr_prot: &UBTP10, info: &UInfoHash, curr_id: &UPeerID) -> IoResult<()> {
        let mut buf = BufWriter::new(buf);
        
        try!(buf.write_u8(curr_prot.len() as u8));
        try!(buf.write(curr_prot));
        
        // Reserved Bytes
        try!(buf.write([0u8; 8].as_slice()));
        
        try!(buf.write(info));
        try!(buf.write(curr_id));
        
        Ok(())
    }
}