//! Establishing a secure connection with a peer.

use std::io::{BufReader, BufWriter, BufferedStream, IoResult, IoError, Closed};
use std::default::{Default};
use std::path::{BytesContainer};
use std::io::net::tcp::{TcpStream};
use std::io::net::ip::{ToSocketAddr};

use peer::{Peer};
use types::{self, PeerID, InfoHash};
use util::{Choice};

const BTP_10_PROTOCOL_LEN: usize = 19;
const BTP_10_PROTOCOL: &'static str = "BitTorrent protocol";

const BTP_10_HANDSHAKE_LEN: usize = 68;

/// A struct representing a handshake that has successfully taken place.
pub struct Handshake {
    conn:      TcpStream,
    remote_id: PeerID
}

/// The entry point for connecting with remote peers.
impl Handshake {
    /// Initiates a handshake with the recipient sending the designated info hash and peer id.
    /// If the response is malformed, the peer sends a different info hash, or the peer sends
    /// us a peer id that we are already using, the handshake will fail.
    ///
    /// This is a blocking operation.
    pub fn initiate<T, S>(recipient: S, info: &InfoHash, curr_id: &PeerID, valid_peer_id: T) 
        -> IoResult<Handshake> where T: for<'a> Fn<(&'a PeerID,), bool>, S: ToSocketAddr {
        let mut conn = try!(TcpStream::connect(try!(recipient.to_socket_addr())));
        let mut handshake = [0u8; BTP_10_HANDSHAKE_LEN];
        
        try!(Handshake::write_handshake(&mut handshake, BTP_10_PROTOCOL, info, curr_id));
        try!(conn.write(&handshake));
        
        try!(conn.read_at_least(handshake.len(), &mut handshake));
        let peer_id = try!(Handshake::verify_handshake(&handshake, BTP_10_PROTOCOL, info, valid_peer_id));
        
        Ok(Handshake{ conn: conn,
            remote_id: peer_id }
        )
    }
    
    /// Completes a handshake that was initiated by the remote peer. If the handshake initiated
    /// by the peer is malformed, the peer sent us a different info hash, or the peer sent us a
    /// peer id that we are already using, the handshake will fail.
    pub fn complete<T>(mut initiater: TcpStream, info: &InfoHash, curr_id: &PeerID, valid: T)
        -> IoResult<Handshake> where T: for<'a> Fn<(&'a PeerID,), bool> {
        let mut handshake = [0u8; BTP_10_HANDSHAKE_LEN];
        
        try!(initiater.read_at_least(handshake.len(), &mut handshake));
        let peer_id = try!(Handshake::verify_handshake(&handshake, BTP_10_PROTOCOL, info, valid));
        
        try!(Handshake::write_handshake(&mut handshake, BTP_10_PROTOCOL, info, curr_id));
        try!(initiater.write(&handshake));
        
        Ok(Handshake{ conn: initiater,
            remote_id: peer_id }
        )
    }
    
    /// Consumes the handshake object and creates a peer object with the number of 
    /// pieces in the current torrent set to num_pieces.
    pub fn into_peer(self, num_pieces: u32) -> Peer {
        Peer{ conn_buf: BufferedStream::new(self.conn),
            self_state: Default::default(),
            remote_state: Default::default(),
            remote_id: self.remote_id,
            remote_pieces: Choice::Two(num_pieces) }
    }
    
    fn verify_handshake<T>(bytes: &[u8], curr_prot: &str, info: &InfoHash, valid_peer_id: T)
        -> IoResult<PeerID> where T: for<'a> Fn<(&'a PeerID,), bool> {
        let mut buf = BufReader::new(bytes);
        
        // Verify Handshake Length
        let remote_length = try!(buf.read_u8()) as usize;
        if remote_length != curr_prot.len() {
            return Err(IoError{ kind: Closed, desc: "Handshake Length Not Recognized", detail: None })
        }
        
        // Verify Protocol
        let local_prot = curr_prot.as_bytes();
        let mut remote_prot = [0u8; BTP_10_PROTOCOL_LEN];
        try!(buf.read_at_least(remote_prot.len(), &mut remote_prot));
        if !remote_prot.iter().zip(local_prot.iter()).all(|(a,b)| a == b) {
            return Err(IoError{ kind: Closed, desc: "Different Protocols In Use", detail: None })
        }
        
        // Reserved Bytes
        try!(buf.read_be_u64());
        
        // Verify InfoHash
        let mut remote_info = [0u8; types::INFO_HASH_LEN];
        try!(buf.read_at_least(remote_info.len(), &mut remote_info));
        if !info.as_slice().iter().zip(remote_info.iter()).all(|(a,b)| a == b) {
            return Err(IoError{ kind: Closed, desc: "Different Info Hash In Use", detail: None })
        }
        
        // Try To Convert Bytes To String Slice
        let mut peer_id_bytes = [0u8; types::PEER_ID_LEN];
        try!(buf.read_at_least(peer_id_bytes.len(), &mut peer_id_bytes));
        let peer_id_str = try!(peer_id_bytes.as_slice().container_as_str().ok_or(
            IoError{ kind: Closed, desc: "Remote Peer ID Is Not Valid UTF-8", detail: None }
        ));
        
        // Verify PeerID
        let peer_id = try!(PeerID::from_str(peer_id_str).ok_or(
            IoError{ kind: Closed, desc: "Remote Peer ID Is Not Valid UTF-8", detail: None }
        ));
        if !valid_peer_id(&peer_id) {
            return Err(IoError{ kind: Closed, desc: "Remote Peer ID In Use", detail: None })
        }
        
        Ok(peer_id)
    }
    
    fn write_handshake(buf: &mut [u8], curr_prot: &str, info: &InfoHash, curr_id: &PeerID)
        -> IoResult<()> {
        let mut buf = BufWriter::new(buf);
        
        try!(buf.write_u8(curr_prot.len() as u8));
        try!(buf.write_str(curr_prot));
        
        // Reserved Bytes
        try!(buf.write([0u8; 8].as_slice()));
        
        try!(buf.write(info.as_slice()));
        try!(buf.write(curr_id.as_slice()));
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use types::{PeerID, InfoHash};
    use std::path::{BytesContainer};
    use std::io::{BufReader};
    use super::{Handshake, BTP_10_PROTOCOL, BTP_10_HANDSHAKE_LEN};

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
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
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
        if !peer_id.as_slice().iter().all(|&a| buf_reader.read_u8().unwrap() == a) {
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
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
    }
    
    #[test]
    fn positive_verify_handshake() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        let local_id = valid_peer_id();
        Handshake::verify_handshake(buffer.as_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), |&: remote_id| {
                let (local_slice, remote_slice) = (local_id.as_slice(), remote_id.as_slice());
            
                local_slice.len() == remote_slice.len() && 
                remote_slice.iter().zip(local_slice.iter()).all(|(&a,&b)| a == b)
        }).unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_invalid_peer_id() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        Handshake::verify_handshake(buffer.as_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), |&: _| false)
        .unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_invalid_info_hash() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        Handshake::verify_handshake(buffer.as_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_two(), |&: _| true)
        .unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_diff_protocol() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        let same_len_protocol = "81tt0rr3nt pr0t0c01";
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        Handshake::verify_handshake(buffer.as_slice(), same_len_protocol,
            &valid_info_hash_one(), |&: _| true )
        .unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_diff_protocol_len() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        let different_len_protocol = "Bittorrent protocol in Rust!!!";
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL,
            &valid_info_hash_one(), &valid_peer_id())
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        Handshake::verify_handshake(buffer.as_slice(), different_len_protocol,
            &valid_info_hash_one(), |&: _| true )
        .unwrap();
    }
}