//! Establishing a secure connection with a peer.

// ADD SUPPORT FOR BEP-0010 EXTENSION PROTOCOL

use std::old_io::{BufReader, BufWriter, BufferedStream, IoResult, IoError, Closed};
use std::default::{Default};
use std::old_path::{BytesContainer};
use std::old_io::net::tcp::{TcpStream};
use std::old_io::net::ip::{ToSocketAddr};

use peer::{Peer};
use types::{self, PeerID, InfoHash};
use util::{Choice};

const BTP_10_PROTOCOL_LEN: usize        = 19;
const BTP_10_PROTOCOL:     &'static str = "BitTorrent protocol";

const BTP_10_HANDSHAKE_LEN: usize = 68;

/// A struct containing information required to make a connection with a remote peer.
#[derive(Copy)]
pub struct Handshaker {
    peer_id:    PeerID,
    info_hash:  InfoHash,
    max_pieces: u32
}

impl Handshaker {
    /// Creates a new Handshaker object that can be used to create connections
    /// with remote peers.
    pub fn new(id: PeerID, hash: InfoHash, max_pieces: u32) -> Handshaker {
        Handshaker{ peer_id: id, info_hash: hash, max_pieces: max_pieces }
    }
    
    /// Initiates a handshake with the recipient. If the recipient responds with
    /// a different info hash or respondes with a peer id we are already using, the 
    /// connection will be dropped. The closure passed in should accept a PeerID 
    /// and return true if it is not in use by another peer we are connected to.
    ///
    /// This is a blocking operation.
    pub fn initiate<S, T>(&self, recipient: S, valid_peer_id: T) -> IoResult<Peer>
        where T: for<'a> Fn(&'a PeerID) -> bool, S: ToSocketAddr {
        let mut conn = try!(TcpStream::connect(try!(recipient.to_socket_addr())));
        let mut handshake_bytes = [0u8; BTP_10_HANDSHAKE_LEN];
        
        try!(write_handshake(&mut handshake_bytes, BTP_10_PROTOCOL, &self.info_hash, &self.peer_id));
        try!(conn.write_all(&handshake_bytes));
        
        try!(conn.read_at_least(handshake_bytes.len(), &mut handshake_bytes));
        let remote_id = try!(
            verify_handshake(&handshake_bytes, BTP_10_PROTOCOL, &self.info_hash, valid_peer_id)
        );
        
        Ok(Peer{ conn_buf: BufferedStream::new(conn),
            self_state: Default::default(),
            remote_state: Default::default(),
            remote_id: remote_id,
            remote_pieces: Choice::Two(self.max_pieces) }
        )
    }
    
    /// Completes a handshake that was initiated by the remote peer. If the initiator
    /// is using a different info hash or is using a peer id that we are already using,
    /// the connection will be dropped. The closure passed in should accept a PeerID
    /// and return true if it is not in use by another peer we are connected to.
    pub fn complete<T>(&self, mut initiator: TcpStream, valid_peer_id: T) -> IoResult<Peer>
        where T: for<'a> Fn(&'a PeerID) -> bool {
        let mut handshake_bytes = [0u8; BTP_10_HANDSHAKE_LEN];
        
        try!(initiator.read_at_least(handshake_bytes.len(), &mut handshake_bytes));
        let remote_id = try!(
            verify_handshake(&handshake_bytes, BTP_10_PROTOCOL, &self.info_hash, valid_peer_id)
        );
        
        try!(write_handshake(&mut handshake_bytes, BTP_10_PROTOCOL, &self.info_hash, &self.peer_id));
        try!(initiator.write_all(&handshake_bytes));
        
        Ok(Peer{ conn_buf: BufferedStream::new(initiator),
            self_state: Default::default(),
            remote_state: Default::default(),
            remote_id: remote_id,
            remote_pieces: Choice::Two(self.max_pieces) }
        )
    }
}

fn verify_handshake<T>(bytes: &[u8], curr_prot: &str, info: &InfoHash, valid_peer_id: T)
    -> IoResult<PeerID> where T: for<'a> Fn(&'a PeerID) -> bool {
    let mut buf_reader = BufReader::new(bytes);
    
    // Verify Handshake Length
    let remote_length = try!(buf_reader.read_u8()) as usize;
    if remote_length != curr_prot.len() {
        return Err(IoError{ kind: Closed, desc: "Handshake Length Not Recognized", detail: None })
    }
    
    // Verify Protocol
    let local_prot = curr_prot.as_bytes();
    let mut remote_prot = [0u8; BTP_10_PROTOCOL_LEN];
    try!(buf_reader.read_at_least(remote_prot.len(), &mut remote_prot));
    if !remote_prot.iter().zip(local_prot.iter()).all(|(a,b)| a == b) {
        return Err(IoError{ kind: Closed, desc: "Different Protocols In Use", detail: None })
    }
    
    // Reserved Bytes
    try!(buf_reader.read_be_u64());
    
    // Verify InfoHash
    let mut remote_info = [0u8; types::INFO_HASH_LEN];
    try!(buf_reader.read_at_least(remote_info.len(), &mut remote_info));
    if !info.as_slice().iter().zip(remote_info.iter()).all(|(a,b)| a == b) {
        return Err(IoError{ kind: Closed, desc: "Different Info Hash In Use", detail: None })
    }
    
    // Try To Convert Bytes To String Slice
    let mut peer_id_bytes = [0u8; types::PEER_ID_LEN];
    try!(buf_reader.read_at_least(peer_id_bytes.len(), &mut peer_id_bytes));
    let peer_id_str = try!(peer_id_bytes.as_slice().container_as_str().ok_or(
        IoError{ kind: Closed, desc: "Remote Peer ID Is Not Valid UTF-8", detail: None }
    ));
    
    // Verify PeerID
    let peer_id = try!(PeerID::from_str(peer_id_str).ok_or(
        IoError{ kind: Closed, desc: "Woops, PeerID Length Error In Library!!!", detail: None }
    ));
    if !valid_peer_id(&peer_id) {
        return Err(IoError{ kind: Closed, desc: "Remote Peer ID In Use", detail: None })
    }
    
    Ok(peer_id)
}

fn write_handshake(bytes: &mut [u8], curr_prot: &str, info: &InfoHash, curr_id: &PeerID) 
    -> IoResult<()> {
    let mut buf_writer = BufWriter::new(bytes);
    
    try!(buf_writer.write_u8(curr_prot.len() as u8));
    try!(buf_writer.write_str(curr_prot));
    
    // Reserved Bytes
    try!(buf_writer.write_all([0u8; 8].as_slice()));
    
    try!(buf_writer.write_all(info.as_slice()));
    try!(buf_writer.write_all(curr_id.as_bytes()));
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use types::{PeerID, InfoHash};
    use std::old_path::{BytesContainer};
    use std::old_io::{BufReader};
    use super::{BTP_10_PROTOCOL, BTP_10_HANDSHAKE_LEN};

    fn valid_peer_id() -> PeerID {
        const VALID_PEER_ID: &'static str = "bittorrent-rs_a52dez";
        
        PeerID::from_str(VALID_PEER_ID).unwrap()
    }
    
    fn valid_info_hash_one() -> InfoHash {
        const VALID_INFO_HASH_ONE: &'static [u8] = b"\xcf\x23\xdf\x22\x07\xd9\x9a\x74\xfb\xe1\x69\xe3\xeb\xa0\x35\xe6\x33\xb6\x5d\x94";
        
        InfoHash::from_bytes(VALID_INFO_HASH_ONE).unwrap()
    }
    
    fn valid_info_hash_two() -> InfoHash {
        const VALID_INFO_HASH_TWO: &'static [u8] = b"\xd0\xd1\x4c\x92\x6e\x6e\x99\x76\x1a\x2f\xdc\xff\x27\xb4\x03\xd9\x63\x76\xef\xf6";
        
        InfoHash::from_bytes(VALID_INFO_HASH_TWO).unwrap()
    }
    
    #[test]
    fn positive_write_handshake() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        super::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        if buf_reader.read_u8().unwrap() as usize != BTP_10_PROTOCOL.len() {
            panic!("Write Failed For Name Length")
        }
        
        let protocol_bytes = buf_reader.read_exact(BTP_10_PROTOCOL.len()).unwrap();
        if protocol_bytes.container_as_str().unwrap() != BTP_10_PROTOCOL {
            panic!("Wrong Failed For Protocol")
        }
        
        buf_reader.read_exact(8).unwrap();
        
        let info_hash = valid_info_hash_one();
        if !info_hash.as_slice().iter().all(|&a| buf_reader.read_u8().unwrap() == a) {
            panic!("Write Failed For Info Hash")
        }
        
        let peer_id = valid_peer_id();
        if !peer_id.as_bytes().iter().all(|&a| buf_reader.read_u8().unwrap() == a) {
            panic!("Write Failed For Peer ID")
        }
        
        if !buf_reader.eof() {
            panic!("Write Wrote Extra Bytes")
        }
    }
    
    #[test]
    #[should_fail]
    fn negative_write_handshake_buffer_too_small() {
        let mut buffer = [0u8; 1];
        super::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
    }
    
    #[test]
    fn positive_verify_handshake() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        super::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        let local_id = valid_peer_id();
        super::verify_handshake(buffer.as_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), |&: remote_id| {
                let (local_slice, remote_slice) = (local_id.as_bytes(), remote_id.as_bytes());
            
                local_slice.len() == remote_slice.len() && 
                remote_slice.iter().zip(local_slice.iter()).all(|(&a,&b)| a == b)
        }).unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_invalid_peer_id() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        super::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        super::verify_handshake(buffer.as_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), |&: _| false)
        .unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_invalid_info_hash() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        super::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        super::verify_handshake(buffer.as_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_two(), |&: _| true)
        .unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_diff_protocol() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        let same_len_protocol = "81tt0rr3nt pr0t0c01";
        super::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        super::verify_handshake(buffer.as_slice(), same_len_protocol,
            &valid_info_hash_one(), |&: _| true )
        .unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_diff_protocol_len() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        let different_len_protocol = "Bittorrent protocol in Rust!!!";
        super::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        super::verify_handshake(buffer.as_slice(), different_len_protocol,
            &valid_info_hash_one(), |&: _| true )
        .unwrap();
    }
}