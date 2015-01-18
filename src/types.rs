use std::slice::{AsSlice};
use std::str::{Str};
use std::path::{BytesContainer};

pub const PEER_ID_LEN: usize = 20;

/// Represents a Peer ID which is a UTF-8 sequence of PEER_ID_LEN characters.
#[derive(Copy)]
pub struct PeerID {
	id: [u8; PEER_ID_LEN]
}

impl PeerID {
    /// Creates a PeerID struct.
    ///
    /// Returns None if the supplied id is too long or too short.
	pub fn from_str<T>(id: T) -> Option<PeerID>
		where T: Str {
        let bytes_slice = id.as_slice().as_bytes();
        let mut peer_id = [0u8; PEER_ID_LEN];
        
        if bytes_slice.len() != PEER_ID_LEN {
            return None
        }
        
        bytes_slice.iter().zip(peer_id.iter_mut()).map(|(src,dst)|
            *dst = *src
        ).count();
        
        Some(PeerID{ id: peer_id })
	}
}

impl AsSlice<u8> for PeerID {
	fn as_slice<'a>(&'a self) -> &'a [u8] {
		self.id.as_slice()
	}
}

pub const INFO_HASH_LEN: usize = 20;

/// Represents an Info Hash which is a byte sequence of length INFO_HASH_LEN;
#[derive(Copy)]
pub struct InfoHash {
	hash: [u8; INFO_HASH_LEN]
}

impl InfoHash {
    /// Creates an InfoHash struct.
    ///
    /// Returns None if the supplied hash is too long or too short.
	pub fn from_bytes<T>(hash: T) -> Option<InfoHash>
		where T: BytesContainer {
        let bytes_slice = hash.container_as_bytes();
        let mut info_hash = [0u8; INFO_HASH_LEN];
        
        if bytes_slice.len() != INFO_HASH_LEN {
            return None
        }
        
		bytes_slice.iter().zip(info_hash.iter_mut()).map( |(src, dst)|
			*dst = *src
		).count();
		
		Some(InfoHash{ hash: info_hash })
	}
}

impl AsSlice<u8> for InfoHash {
	fn as_slice<'a>(&'a self) -> &'a [u8] {
		self.hash.as_slice()
	}
}