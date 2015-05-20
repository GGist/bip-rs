//! Error types used by the library.

use std::error::{Error};
use std::fmt::{self, Display, Formatter, Debug};
use std::result::{Result};

pub type TorrentResult<T> = Result<T, TorrentError>;

/// A list specifying the types of TorrentErrors that may occur.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TorrentErrorKind {
    /// A Mandatory Key Is Missing In The File.
    MissingKey,
    /// A Value Was Found That Has The Wrong Type.
    WrongType,
    /// Some Other Error, Possibly Converted From Another Type.
    Other
}

/// A type for specifying errors when reading a torrent file.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TorrentError {
    pub kind: TorrentErrorKind,
    pub desc: &'static str,
    pub detail: Option<String>
}

impl Display for TorrentError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        try!(f.write_str("Kind: "));
        try!(Debug::fmt(&self.kind, f));
        
        try!(f.write_str(" Description: "));
        try!(f.write_str(self.desc));
        
        try!(f.write_str(" Detail: "));
        match self.detail {
            Some(ref n) => try!(f.write_str(n)),
            None        => ()
        };
        
        Ok(())
    }   
}

impl Error for TorrentError {
    fn description(&self) -> &str { self.desc }
    
    fn cause(&self) -> Option<&Error> { None }
}