//! Interface for hashing functionality.

use crypto::sha1::{Sha1};
use crypto::digest::{Digest};

pub const SHA1_HASH_LEN: usize = 20;

pub fn apply_sha1(src: &[u8], dst: &mut [u8]) {
    let mut sha = Sha1::new();
    
    sha.input(src);
    
    sha.result(dst);
}