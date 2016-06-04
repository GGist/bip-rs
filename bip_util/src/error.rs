/// Result type for a `LengthError`.
pub type LengthResult<T> = Result<T, LengthError>;

/// Enumeraters a set of length related errors.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum LengthErrorKind {
    /// Length exceeded an expected size.
    LengthExceeded,
    /// Length is not equal to an expected size.
    LengthExpected,
    /// Length is not a multiple of an expected size.
    LengthMultipleExpected,
}

/// Generic length error for various types.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct LengthError {
    kind: LengthErrorKind,
    length: usize,
    index: Option<usize>,
}

impl LengthError {
    /// Create a `LengthError`.
    pub fn new(kind: LengthErrorKind, length: usize) -> LengthError {
        LengthError {
            kind: kind,
            length: length,
            index: None,
        }
    }

    /// Create a `LengthError` for a given element index.
    pub fn with_index(kind: LengthErrorKind, length: usize, index: usize) -> LengthError {
        LengthError {
            kind: kind,
            length: length,
            index: Some(index),
        }
    }

    /// Error is with the given length/length multiple.
    pub fn length(&self) -> usize {
        self.length
    }

    /// Error is for the element at the given index.
    pub fn index(&self) -> Option<usize> {
        self.index
    }
}
