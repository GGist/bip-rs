use crypto::digest::Digest;
use crypto::sha1::Sha1;

use sha::{self, ShaHash};

/// Building `ShaHash` objects by adding byte slices to the hash.
#[derive(Clone)]
pub struct ShaHashBuilder {
    sha: Sha1,
}

impl ShaHashBuilder {
    /// Create a new `ShaHashBuilder`.
    pub fn new() -> ShaHashBuilder {
        ShaHashBuilder { sha: Sha1::new() }
    }

    /// Add bytes to the `ShaHashBuilder`.
    pub fn add_bytes(mut self, bytes: &[u8]) -> ShaHashBuilder {
        self.sha.input(bytes);

        self
    }

    /// Build the ShaHash from the `ShaHashBuilder`.
    pub fn build(&self) -> ShaHash {
        let mut buffer = [0u8; sha::SHA_HASH_LEN];

        self.sha.clone().result(&mut buffer);

        buffer.into()
    }
}
