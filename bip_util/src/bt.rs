use sha;

/// Bittorrent NodeId.
pub type NodeId = sha::ShaHash;

/// Bittorrent PeerId.
pub type PeerId = sha::ShaHash;

/// Bittorrent InfoHash.
pub type InfoHash = sha::ShaHash;

/// Length of a NodeId.
pub const NODE_ID_LEN: usize = sha::SHA_HASH_LEN;

/// Length of a PeerId.
pub const PEER_ID_LEN: usize = sha::SHA_HASH_LEN;

/// Length of an InfoHash.
pub const INFO_HASH_LEN: usize = sha::SHA_HASH_LEN;
