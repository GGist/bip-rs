#![feature(ip_addr)]
//! Utilities used by the Bittorrent Infrastructure Project.

extern crate sha1;
extern crate rand;
extern crate chrono;

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

//----------------------------------------------------------------------------//

use std::net::{SocketAddr, Ipv4Addr, SocketAddrV4};

/// Get the default route ipv4 socket.
pub fn default_route_v4() -> SocketAddr {
    let v4_addr = Ipv4Addr::new(0, 0, 0, 0);
    let v4_sock = SocketAddrV4::new(v4_addr, 0);
    
    SocketAddr::V4(v4_sock)
}

//----------------------------------------------------------------------------//

/// Applies a Fisher-Yates shuffle on the given list.
pub fn fisher_shuffle<T: Copy>(list: &mut [T]) {
    for i in 0..list.len() {
        let swap_index = (rand::random::<usize>() % (list.len() - i)) + i;
        
        let temp = list[i];
        list[i] = list[swap_index];
        list[swap_index] = temp;
    }
}

#[cfg(test)]
mod tests {
    
    #[test]
    fn positive_fisher_shuffle() {
        let mut test_slice = [1, 2, 3, 4];
        
        super::fisher_shuffle(&mut test_slice);
        
        assert!(test_slice.contains(&1));
        assert!(test_slice.contains(&2));
        assert!(test_slice.contains(&3));
        assert!(test_slice.contains(&4));
    }
}