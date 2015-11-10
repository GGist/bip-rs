/// Result type for GenericError types.
pub type GenericResult<T> = Result<T, GenericError>;

/// Enumeraters a set of generic errors for data validation.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum GenericError {
    /// An Invalid Length Was Given
    ///
    /// Expected (length) provided
    InvalidLength(usize),
    /// An Invalid Length Multiple Was Given
    ///
    /// Expected (multiple) provided
    InvalidLengthMultiple(usize),
    /// An Element Has An Invalid Length
    ///
    /// Expected (index, length) provided
    InvalidElementLength(usize, usize)
}