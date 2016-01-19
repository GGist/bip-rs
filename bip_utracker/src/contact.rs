use std::borrow::{Cow};
use std::io::{self, Write};
use std::net::{SocketAddrV4, SocketAddrV6};

use bip_util::convert::{self};
use nom::{IResult, Needed};

const SOCKET_ADDR_V4_BYTES: usize = 6;
const SOCKET_ADDR_V6_BYTES: usize = 18;

/// Container for peers to be sent/received from a tracker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompactPeers<'a> {
    V4(CompactPeersV4<'a>),
    V6(CompactPeersV6<'a>)
}

impl<'a> CompactPeers<'a> {
    /// Construct a CompactPeers::V4 from the given bytes.
    pub fn from_bytes_v4(bytes: &'a [u8]) -> IResult<&'a [u8], CompactPeers<'a>> {
        match CompactPeersV4::from_bytes(bytes) {
            IResult::Done(i, peers)  => IResult::Done(i, CompactPeers::V4(peers)),
            IResult::Error(err)      => IResult::Error(err),
            IResult::Incomplete(need) => IResult::Incomplete(need)
        }
    }
    
    /// Construct a CompactPeers::V6 from the given bytes.
    pub fn from_bytes_v6(bytes: &'a [u8]) -> IResult<&'a [u8], CompactPeers<'a>> {
        match CompactPeersV6::from_bytes(bytes) {
            IResult::Done(i, peers)   => IResult::Done(i, CompactPeers::V6(peers)),
            IResult::Error(err)       => IResult::Error(err),
            IResult::Incomplete(need) => IResult::Incomplete(need)
        }
    }
    
    /// Write the underlying CompactPeers to the given writer.
    pub fn write_bytes<W>(&self, writer: W) -> io::Result<()>
        where W: Write {
        match self {
            &CompactPeers::V4(ref peers) => peers.write_bytes(writer),
            &CompactPeers::V6(ref peers) => peers.write_bytes(writer)
        }
    }
}

/// Container for IPv4 peers to be sent/received from a tracker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactPeersV4<'a> {
    peers: Cow<'a, [u8]>
}

impl<'a> CompactPeersV4<'a> {
    /// Create a new CompactPeersV4.
    pub fn new() -> CompactPeersV4<'a> {
        CompactPeersV4{ peers: Cow::Owned(Vec::new()) }
    }
    
    /// Construct a CompactPeersV4 from the given bytes.
    pub fn from_bytes(bytes: &'a [u8]) -> IResult<&'a [u8], CompactPeersV4<'a>> {
        parse_peers_v4(bytes)
    }
    
    /// Write the CompactPeersV4 to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        try!(writer.write_all(&*self.peers));
        
        Ok(())
    }
    
    /// Add the given peer to the list of peers.
    pub fn insert(&mut self, peer: SocketAddrV4) {
        let peer_bytes = convert::sock_v4_to_bytes_be(peer);
        
        self.peers.to_mut().extend_from_slice(&peer_bytes);
    }
    
    /// Iterator over all of the contact information.
    pub fn iter<'b>(&'b self) -> CompactPeersV4Iter<'b> {
        CompactPeersV4Iter::new(&*self.peers)
    }
}

fn parse_peers_v4<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], CompactPeersV4<'a>> {
    let remainder_bytes = bytes.len() % SOCKET_ADDR_V4_BYTES;

    if remainder_bytes != 0 {
        IResult::Incomplete(Needed::Size(SOCKET_ADDR_V4_BYTES - remainder_bytes))
    } else {
        let end_of_bytes = &bytes[bytes.len()..bytes.len()];
    
        IResult::Done(end_of_bytes, CompactPeersV4{ peers: Cow::Borrowed(bytes) })
    }
}

//----------------------------------------------------------------------------//

/// Iterator over the SocketAddrV4 info for some peers.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CompactPeersV4Iter<'a> {
    peers:  &'a [u8],
    offset: usize
}

impl<'a> CompactPeersV4Iter<'a> {
    /// Create a new CompactPeersV4Iter.
    fn new(peers: &'a [u8]) -> CompactPeersV4Iter<'a> {
        CompactPeersV4Iter{ peers: peers, offset: 0 }
    }
}

impl<'a> Iterator for CompactPeersV4Iter<'a> {
    type Item = SocketAddrV4;
    
    fn next(&mut self) -> Option<SocketAddrV4> {
        if self.offset == self.peers.len() {
            None
        } else {
            let mut sock_bytes = [0u8; SOCKET_ADDR_V4_BYTES];
            
            for (src, dst) in self.peers.iter()
                .skip(self.offset)
                .take(SOCKET_ADDR_V4_BYTES)
                .zip(sock_bytes.iter_mut()) {
                *dst = *src;
            }
            self.offset += SOCKET_ADDR_V4_BYTES;
            
            Some(convert::bytes_be_to_sock_v4(sock_bytes))
        }
    }
}

//----------------------------------------------------------------------------//

/// Container for IPv6 peers to be sent/received from a tracker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactPeersV6<'a> {
    peers: Cow<'a, [u8]>
}

impl<'a> CompactPeersV6<'a> {
    /// Create a new CompactPeersV6.
    pub fn new() -> CompactPeersV6<'a> {
        CompactPeersV6{ peers: Cow::Owned(Vec::new()) }
    }
    
    /// Construct a CompactPeersV6 from the given bytes.
    pub fn from_bytes(bytes: &'a [u8]) -> IResult<&'a [u8], CompactPeersV6<'a>> {
        parse_peers_v6(bytes)
    }
    
    /// Write the CompactPeersV6 to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        try!(writer.write_all(&*self.peers));
        
        Ok(())
    }
    
    /// Add the given peer to the list of peers.
    pub fn insert(&mut self, peer: SocketAddrV6) {
        let peer_bytes = convert::sock_v6_to_bytes_be(peer);
        
        self.peers.to_mut().extend_from_slice(&peer_bytes);
    }
    
    /// Iterator over all of the contact information.
    pub fn iter<'b>(&'b self) -> CompactPeersV6Iter<'b> {
        CompactPeersV6Iter::new(&*self.peers)
    }
}

fn parse_peers_v6<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], CompactPeersV6<'a>> {
    let remainder_bytes = bytes.len() % SOCKET_ADDR_V6_BYTES;

    if remainder_bytes != 0 {
        IResult::Incomplete(Needed::Size(SOCKET_ADDR_V6_BYTES - remainder_bytes))
    } else {
        let end_of_bytes = &bytes[bytes.len()..bytes.len()];
    
        IResult::Done(end_of_bytes, CompactPeersV6{ peers: Cow::Borrowed(bytes) })
    }
}

//----------------------------------------------------------------------------//

/// Iterator over the SocketAddrV6 info for some peers.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CompactPeersV6Iter<'a> {
    peers:  &'a [u8],
    offset: usize
}

impl<'a> CompactPeersV6Iter<'a> {
    /// Create a new CompactPeersV6Iter.
    fn new(peers: &'a [u8]) -> CompactPeersV6Iter<'a> {
        CompactPeersV6Iter{ peers: peers, offset: 0 }
    }
}

impl<'a> Iterator for CompactPeersV6Iter<'a> {
    type Item = SocketAddrV6;
    
    fn next(&mut self) -> Option<SocketAddrV6> {
        if self.offset == self.peers.len() {
            None
        } else {
            let mut sock_bytes = [0u8; SOCKET_ADDR_V6_BYTES];
            
            for (src, dst) in self.peers.iter()
                .skip(self.offset)
                .take(SOCKET_ADDR_V6_BYTES)
                .zip(sock_bytes.iter_mut()) {
                *dst = *src;
            }
            self.offset += SOCKET_ADDR_V6_BYTES;
            
            Some(convert::bytes_be_to_sock_v6(sock_bytes))
        }
    }
}

#[cfg(test)]
mod tests {
    use nom::{IResult};

    use super::{CompactPeersV4, CompactPeersV6, CompactPeersV4Iter, CompactPeersV6Iter};

    #[test]
    fn positive_iterate_v4() {
        let mut peers = CompactPeersV4::new();
        
        let peer_one = "127.0.0.1:2354".parse().unwrap();
        let peer_two = "10.0.0.5:3245".parse().unwrap();
        
        peers.insert(peer_one);
        peers.insert(peer_two);
        
        let mut peers_iter = peers.iter();
        
        assert_eq!(peers_iter.next(), Some(peer_one));
        assert_eq!(peers_iter.next(), Some(peer_two));
        assert_eq!(peers_iter.next(), None);
    }

    #[test]
    fn positive_parse_empty_v4() {
        let bytes = [];
        
        let received = CompactPeersV4::from_bytes(&bytes);
        let expected = CompactPeersV4::new();
        
        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_peer_v4() {
        let bytes = [127, 0, 0, 1, 0, 15];
        
        let received = CompactPeersV4::from_bytes(&bytes);
        let mut expected = CompactPeersV4::new();
        
        expected.insert("127.0.0.1:15".parse().unwrap());
        
        assert_eq!(received, IResult::Done(&b""[..], expected));
    }
    
    #[test]
    fn positive_parse_peers_v4() {
        let bytes = [127, 0, 0, 1, 0, 15, 127, 0, 0, 1, 1, 0];
        
        let received = CompactPeersV4::from_bytes(&bytes);
        let mut expected = CompactPeersV4::new();
        
        expected.insert("127.0.0.1:15".parse().unwrap());
        expected.insert("127.0.0.1:256".parse().unwrap());
        
        assert_eq!(received, IResult::Done(&b""[..], expected));
    }
    
    #[test]
    fn positive_write_empty_v4() {
        let mut received = Vec::new();
        
        let peers = CompactPeersV4::new();
        peers.write_bytes(&mut received).unwrap();
        
        let expected = [];
        
        assert_eq!(&received[..], &expected[..]);
    }
    
    #[test]
    fn positive_write_peer_v4() {
        let mut received = Vec::new();
        
        let mut peers = CompactPeersV4::new();
        peers.insert("127.0.0.1:256".parse().unwrap());
        peers.write_bytes(&mut received).unwrap();
        
        let expected = [127, 0, 0, 1, 1, 0];
        
        assert_eq!(&received[..], &expected[..]);
    }
    
    #[test]
    fn positive_write_peers_v4() {
        let mut received = Vec::new();
        
        let mut peers = CompactPeersV4::new();
        peers.insert("127.0.0.1:256".parse().unwrap());
        peers.insert("127.0.0.1:0".parse().unwrap());
        peers.write_bytes(&mut received).unwrap();
        
        let expected = [127, 0, 0, 1, 1, 0, 127, 0, 0, 1, 0, 0];
        
        assert_eq!(&received[..], &expected[..]);
    }
    
    #[test]
    fn positive_iterate_v6() {
        let mut peers = CompactPeersV6::new();
        
        let peer_one = "[ADBB:234A:55BD:FF34:3D3A:FFFF:234A:55BD]:256".parse().unwrap();
        let peer_two = "[ADBB:0000:55BD:FF34:3D3A::234A:55BD]:3923".parse().unwrap();
        
        peers.insert(peer_one);
        peers.insert(peer_two);
        
        let mut peers_iter = peers.iter();
        
        assert_eq!(peers_iter.next(), Some(peer_one));
        assert_eq!(peers_iter.next(), Some(peer_two));
        assert_eq!(peers_iter.next(), None);
    }
    
    #[test]
    fn positive_parse_empty_v6() {
        let bytes = [];
        
        let received = CompactPeersV6::from_bytes(&bytes);
        let expected = CompactPeersV6::new();
        
        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_peer_v6() {
        let bytes = [0xAD, 0xBB, 0x23, 0x4A, 0x55, 0xBD, 0xFF, 0x34,
            0x3D, 0x3A, 0x00, 0x00, 0x23, 0x4A, 0x55, 0xBD, 1, 0];
        
        let received = CompactPeersV6::from_bytes(&bytes);
        let mut expected = CompactPeersV6::new();
        
        expected.insert("[ADBB:234A:55BD:FF34:3D3A::234A:55BD]:256".parse().unwrap());
        
        assert_eq!(received, IResult::Done(&b""[..], expected));
    }
    
    #[test]
    fn positive_parse_peers_v6() {
        let bytes = [0xAD, 0xBB, 0x23, 0x4A, 0x55, 0xBD, 0xFF, 0x34,
            0x3D, 0x3A, 0x00, 0x00, 0x23, 0x4A, 0x55, 0xBD, 1, 0,
            0xDA, 0xBB, 0x23, 0x4A, 0x55, 0xBD, 0xFF, 0x34,
            0x3D, 0x3A, 0x00, 0x00, 0x23, 0x4A, 0x55, 0xBD, 2, 0];
        
        let received = CompactPeersV6::from_bytes(&bytes);
        let mut expected = CompactPeersV6::new();
        
        expected.insert("[ADBB:234A:55BD:FF34:3D3A::234A:55BD]:256".parse().unwrap());
        expected.insert("[DABB:234A:55BD:FF34:3D3A::234A:55BD]:512".parse().unwrap());
        
        assert_eq!(received, IResult::Done(&b""[..], expected));
    }
    
    #[test]
    fn positive_write_empty_v6() {
        let mut received = Vec::new();
        
        let peers = CompactPeersV6::new();
        peers.write_bytes(&mut received).unwrap();
        
        let expected = [];
        
        assert_eq!(&received[..], &expected[..]);
    }
    
    #[test]
    fn positive_write_peer_v6() {
        let mut received = Vec::new();
        
        let mut peers = CompactPeersV6::new();
        peers.insert("[ADBB:234A:55BD:FF34:3D3A::234A:55BD]:256".parse().unwrap());
        peers.write_bytes(&mut received).unwrap();
        
        let expected = [0xAD, 0xBB, 0x23, 0x4A, 0x55, 0xBD, 0xFF, 0x34,
            0x3D, 0x3A, 0x00, 0x00, 0x23, 0x4A, 0x55, 0xBD, 1, 0];
        
        assert_eq!(&received[..], &expected[..]);
    }
    
    #[test]
    fn positive_write_peers_v6() {
        let mut received = Vec::new();
        
        let mut peers = CompactPeersV6::new();
        peers.insert("[ADBB:234A:55BD:FF34:3D3A::234A:55BD]:256".parse().unwrap());
        peers.insert("[DABB:234A:55BD:FF34:3D3A::234A:55BD]:512".parse().unwrap());
        peers.write_bytes(&mut received).unwrap();
        
        let expected = [0xAD, 0xBB, 0x23, 0x4A, 0x55, 0xBD, 0xFF, 0x34,
            0x3D, 0x3A, 0x00, 0x00, 0x23, 0x4A, 0x55, 0xBD, 1, 0,
            0xDA, 0xBB, 0x23, 0x4A, 0x55, 0xBD, 0xFF, 0x34,
            0x3D, 0x3A, 0x00, 0x00, 0x23, 0x4A, 0x55, 0xBD, 2, 0];
        
        assert_eq!(&received[..], &expected[..]);
    }
}