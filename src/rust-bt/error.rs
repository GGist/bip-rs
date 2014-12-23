use std::io::{IoError};
use std::error::{Error, FromError};
use std::result::{Result};

pub type ParseResult<T> = Result<T, ParseError>;
pub type TorrResult<T> = Result<T, TorrError>;

/// Used when parsing external data that may have errors at any position within
/// the buffer.
///
/// A pos of -1 indicates that this error was converted from another error where 
/// a position value would not make sense.
#[deriving(Show)]
pub struct ParseError {
    pub pos: u64,
    pub desc: &'static str,
    pub detail: Option<String>
}

impl ParseError {
    /// Constructs a new ParseError object where the pos information will get
    /// embedded within the detail value so that it is preserved when accessing
    /// methods via Error or when converting to another error.
    pub fn new(pos: u64, desc: &'static str, detail: Option<String>) -> ParseError {
        let mut more_detail = match detail {
            Some(mut n) => {
                n.push_str(" - Occurred At Position: ");
                n
            },
            None    => String::from_str("Error At Position: ")
        };
        more_detail.push_str(pos.to_string().as_slice());
        
        ParseError{ pos: pos, desc: desc, detail: Some(more_detail) }
    }
}

impl FromError<IoError> for ParseError {
    fn from_error(err: IoError) -> ParseError {
        ParseError{ pos: -1, desc: err.desc, detail: err.detail }
    }
}

impl Error for ParseError {
    fn description(&self) -> &str { self.desc }
    
    fn detail(&self) -> Option<String> { self.detail.clone() }
    
    fn cause(&self) -> Option<&Error> { None }
}

/// A list specifying the types of TorrErrors that may occur.
#[deriving(Show, Copy)]
pub enum TorrErrorKind {
    /// A key is missing in one of the bencoded dictionaries.
    MissingKey,
    /// The data type of one of the bencoded values is wrong.
    WrongType,
    /// An error occurred that is not in this list.
    Other
}

/// Used to raise an error when a piece of data required by the Torrent is 
/// missing from the Bencode data.
#[deriving(Show)]
pub struct TorrError {
    pub kind: TorrErrorKind,
    pub desc: &'static str,
    pub detail: Option<String>
}

impl FromError<IoError> for TorrError {
    fn from_error(err: IoError) -> TorrError {
        TorrError{ kind: TorrErrorKind::Other, desc: err.desc, detail: err.detail }
    }
}

impl Error for TorrError {
    fn description(&self) -> &str { self.desc }
    
    fn detail(&self) -> Option<String> { self.detail.clone() }
    
    fn cause(&self) -> Option<&Error> { None }
}