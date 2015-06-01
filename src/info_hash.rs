use std::ops::{BitXor};

/// Length in bytes of an info hash.
pub const INFO_HASH_LEN: usize = 20;

/// Hash of the info dictionary within a torrent file.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct InfoHash {
    hash: [u8; INFO_HASH_LEN]
}

impl InfoHash {
    /// Create an InfoHash from the given bytes.
    ///
    /// Returns None if the length of the bytes is not equal to INFO_HASH_LEN.
    pub fn from_bytes(bytes: &[u8]) -> Option<InfoHash> {
        let mut buffer = [0u8; INFO_HASH_LEN];
        
        if bytes.len() != INFO_HASH_LEN {
            None
        } else {
            for (src, dst) in bytes.iter().zip(buffer.iter_mut()) {
                *dst = *src;
            }
            
            Some(InfoHash{ hash: buffer })
        }
    }
}

impl From<[u8; INFO_HASH_LEN]> for InfoHash {
    fn from(info_hash: [u8; INFO_HASH_LEN]) -> InfoHash {
        InfoHash{ hash: info_hash }
    }
}

impl BitXor<InfoHash> for InfoHash {
    type Output = InfoHash;
    
    fn bitxor(mut self, rhs: InfoHash) -> InfoHash {
        for (src, dst) in rhs.hash.iter().zip(self.hash.iter_mut()) {
            *dst = *src ^ *dst;
        }
        
        self
    }
}