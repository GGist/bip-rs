//! Utilities used by the Bittorrent Infrastructure Project.

extern crate sha1;
extern crate rand;
extern crate chrono;

/// Bittorrent specific types and functionality.
pub mod bt;

/// Converting data between types.
pub mod convert;

/// Networking primitives and helpers.
pub mod net;

/// SHA-1 wrapper and utilities.
pub mod sha;

/// Testing fixtures for dependant crates.
// TODO: Some non test functions in other crates use this, mark that as cfg test
// when we migrate away from these functions in non test functions.
pub mod test;

mod error;

pub use error::{GenericResult, GenericError};

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