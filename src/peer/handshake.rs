//! Establishing a secure connection with a peer.

use std::io::{BufReader, BufWriter, BufferedStream, IoResult, IoError, Closed};
use std::default::{Default};
use std::io::net::tcp::{TcpStream};
use std::io::net::ip::{SocketAddr};

use peer::{Peer};
use util::{self, SPeerID, UPeerID, UInfoHash, UBTP10, Choice};

// TODO: Need To Add A One Time Check Somewhere That This Is Valid ASCII
const BTP_10_PROTOCOL: &'static str = "BitTorrent protocol";
const BTP_10_HANDSHAKE_LEN: usize = 68;

/// A struct representing a handshake that has successfully taken place.
pub struct Handshake {
    conn: TcpStream,
    remote_id: SPeerID
}

/// The entry point for connecting with remote peers.
impl Handshake {
    /// Initiates a handshake with the recipient sending the designated info hash and peer id.
    /// If the response is malformed, the peer sends a different info hash, or the peer sends
    /// us a peer id that we are already using, the handshake will fail.
    ///
    /// This is a blocking operation.
    pub fn initiate<T>(recipient: SocketAddr, info: &UInfoHash, curr_id: &UPeerID, valid: T) 
        -> IoResult<Handshake> where T: for<'a> Fn<(&'a UPeerID,), bool> {
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
    
    /// Completes a handshake that was initiated by the remote peer. If the handshake initiated
    /// by the peer is malformed, the peer sent us a different info hash, or the peer sent us a
    /// peer id that we are already using, the handshake will fail.
    pub fn complete<T>(mut initiater: TcpStream, info: &UInfoHash, curr_id: &UPeerID, valid: T)
        -> IoResult<Handshake> where T: for<'a> Fn<(&'a UPeerID,), bool> {
        let mut handshake = [0u8; BTP_10_HANDSHAKE_LEN];
        
        try!(initiater.read_at_least(handshake.len(), &mut handshake));
        let peer_id = try!(Handshake::verify_handshake(&handshake, BTP_10_PROTOCOL.as_bytes(), info, valid));
        
        try!(Handshake::write_handshake(&mut handshake, BTP_10_PROTOCOL.as_bytes(), info, curr_id));
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
    
    fn verify_handshake<T>(bytes: &[u8], curr_prot: &UBTP10, info: &UInfoHash, valid: T)
        -> IoResult<SPeerID> where T: for<'a> Fn<(&'a UPeerID,), bool> {
        let mut buf = BufReader::new(bytes);
        
        let remote_length = try!(buf.read_u8());
        if remote_length as usize != curr_prot.len() {
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

#[cfg(test)]
mod tests {
    use std::path::{BytesContainer};
    use std::io::{BufReader};
    use super::{Handshake, BTP_10_PROTOCOL, BTP_10_HANDSHAKE_LEN};

    const VALID_INFO_HASH_ONE: &'static [u8] = b"\xcf\x23\xdf\x22\x07\xd9\x9a\x74\xfb\xe1\x69\xe3\xeb\xa0\x35\xe6\x33\xb6\x5d\x94";
    const VALID_INFO_HASH_TWO: &'static [u8] = b"\xd0\xd1\x4c\x92\x6e\x6e\x99\x76\x1a\x2f\xdc\xff\x27\xb4\x03\xd9\x63\x76\xef\xf6";
    
    const VALID_PEER_ID: &'static [u8] = b"bittorrent-rs_a52dez";
    
    #[test]
    fn positive_write_handshake() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_ONE, VALID_PEER_ID)
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
        
        for &i in VALID_INFO_HASH_ONE.iter() {
            if i != buf_reader.read_u8().unwrap() {
                panic!("Write Failed For Info Hash")
            }
        }
        
        for &i in VALID_PEER_ID.iter() {
            if i != buf_reader.read_u8().unwrap() {
                panic!("Write Failed For Peer ID")
            }
        }
        
        if !buf_reader.eof() {
            panic!("Write Wrote Extra Bytes")
        }
    }
    
    #[test]
    #[should_fail]
    fn negative_write_handshake_buffer_too_small() {
        let mut buffer = [0u8; 1];
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_ONE, VALID_PEER_ID)
        .unwrap();
    }
    
    #[test]
    fn positive_verify_handshake() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_ONE, VALID_PEER_ID)
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        Handshake::verify_handshake(buffer.as_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_ONE, |&: remote_id| {
            VALID_PEER_ID.len() == remote_id.len() && remote_id.iter().zip(VALID_PEER_ID.iter()).all(|(a, b)| a == b)
        }).unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_invalid_peer_id() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_ONE, VALID_PEER_ID)
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        Handshake::verify_handshake(buffer.as_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_ONE, |&: _| false)
        .unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_invalid_info_hash() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_ONE, VALID_PEER_ID)
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        Handshake::verify_handshake(buffer.as_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_TWO, |&: _| true)
        .unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_diff_protocol() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        let same_len_protocol = "81tt0rr3nt pr0t0c01";
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_ONE, VALID_PEER_ID)
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        Handshake::verify_handshake(buffer.as_slice(), same_len_protocol.as_bytes(),
            VALID_INFO_HASH_ONE, |&: _| true )
        .unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_verify_handshake_diff_protocol_len() {
        let mut buffer = [0u8; BTP_10_HANDSHAKE_LEN];
        let different_len_protocol = "Bittorrent protocol in Rust!!!";
        Handshake::write_handshake(buffer.as_mut_slice(), BTP_10_PROTOCOL.as_bytes(),
            VALID_INFO_HASH_ONE, VALID_PEER_ID)
        .unwrap();
        
        // In closure, just check to make sure our peer id was checked
        Handshake::verify_handshake(buffer.as_slice(), different_len_protocol.as_bytes(),
            VALID_INFO_HASH_ONE, |&: _| true )
        .unwrap();
    }
}