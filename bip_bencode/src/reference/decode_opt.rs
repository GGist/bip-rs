use std::default::Default;

const DEFAULT_MAX_RECURSION:  usize = 50;
const DEFAULT_CHECK_KEY_SORT: bool = false;

/// Stores decoding options for modifying decode behavior.
#[derive(Copy, Clone)]
pub struct BDecodeOpt {
    max_recursion:  usize,
    check_key_sort: bool
}

impl BDecodeOpt {
    /// Create a new `BDecodeOpt` object.
    pub fn new(max_recursion: usize, check_key_sort: bool) -> BDecodeOpt {
        BDecodeOpt{ max_recursion: max_recursion, check_key_sort: check_key_sort }
    }

    /// Maximum limit allowed when decoding bencode.
    pub fn max_recursion(&self) -> usize {
        self.max_recursion
    }

    /// Whether or not an error should be thrown for out of order dictionary keys.
    pub fn check_key_sort(&self) -> bool {
        self.check_key_sort
    }
}

impl Default for BDecodeOpt {
    fn default() -> BDecodeOpt {
        BDecodeOpt::new(DEFAULT_MAX_RECURSION, DEFAULT_CHECK_KEY_SORT)
    }
}