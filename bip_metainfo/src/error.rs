use std::borrow::{Cow};
use std::error::{Error};
use std::io::{self};

use walkdir::{self};

/// Result of parsing a torrent file.
pub type ParseResult<T> = Result<T, ParseError>;

/// Error raised as a result of parsing a torrent file.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ParseError {
    kind:   ParseErrorKind,
    detail: Cow<'static, str>
}

/// Enumerates classes of errors that could have raised a ParseError.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ParseErrorKind {
    /// Data is invalid.
    ///
    /// Tracker url is not valid.
    InvalidData,
    /// Missing required data.
    ///
    /// Parse errors related to required data that is missing.
    MissingData,
    /// Data is corrupted.
    ///
    /// Decode errors related to encoding scheme violations.
    CorruptData,
    /// Some IO related error occured.
    ///
    /// File does not exist, could not read from file.
    IoError
}

impl ParseError {
    pub fn new<D>(kind: ParseErrorKind, detail: D) -> ParseError
        where D: Into<Cow<'static, str>> {
        ParseError{ kind: kind, detail: detail.into() }
    }
    
    pub fn kind(&self) -> ParseErrorKind {
        self.kind
    }
    
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> ParseError {
        ParseError::new(ParseErrorKind::IoError, err.to_string())
    }
}

impl From<walkdir::Error> for ParseError {
    fn from(err: walkdir::Error) -> ParseError {
        ParseError::new(ParseErrorKind::IoError, err.description().to_owned())
    }
}