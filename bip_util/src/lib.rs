//! Utilities used by the Bittorrent Infrastructure Project.

extern crate sha1;
extern crate rand;
extern crate time;

/// Working with and expressing SHA-1 values.
pub mod hash;

/// Testing fixtures for dependant crates.
pub mod test;

mod convert;
mod error;

pub use convert::*;
pub use error::{GenericResult, GenericError};

/// Bittorrent NodeId.
pub type NodeId = hash::ShaHash;

/// Bittorrent PeerId.
pub type PeerId = hash::ShaHash;

/// Bittorrent InfoHash.
pub type InfoHash = hash::ShaHash;

/// Length of a NodeId.
pub const NODE_ID_LEN: usize = hash::SHA_HASH_LEN;

/// Length of a PeerId.
pub const PEER_ID_LEN: usize = hash::SHA_HASH_LEN;

/// Length of an InfoHash.
pub const INFO_HASH_LEN: usize = hash::SHA_HASH_LEN;