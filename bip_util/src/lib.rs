//! Utilities used by the Bittorrent Infrastructure Project.

extern crate crypto;
extern crate num;
extern crate rand;
extern crate chrono;

/// Bittorrent specific types.
pub mod bt;

/// Converting between data.
pub mod convert;

/// Networking primitives and helpers.
pub mod net;

/// Generic sender utilities.
pub mod send;

/// Hash primitives and helpers.
pub mod sha;

/// Testing fixtures for dependant crates.
/// TODO: Some non test functions in other crates use this, mark that as cfg test
/// when we migrate away from these functions in non test functions.
pub mod test;

/// Generating transaction ids.
pub mod trans;

/// Common error types.
pub mod error;

// ----------------------------------------------------------------------------//

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
