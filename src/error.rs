//! Error types used in the library.

use std::borrow::{Cow, ToOwned};
use std::convert::{From};
use std::error::{Error};
use std::fmt::{self, Display, Formatter, Debug};
use std::io::{self};
use std::result::{Result};

pub type BencodeResult<T> = Result<T, BencodeError>;

/// A list specifying categories of BencodeError types.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BencodeErrorKind {
    /// An Incomplete Number Of Bytes.
    BytesEmpty,
    /// An Invalid Byte Was Found.
    ///
    /// Position Of Invalid Byte Has Been Provided.
    InvalidByte,
    /// An Invalid Integer Was Found.
    InvalidInt,
    /// An Invalid Key Was Found.
    InvalidKey,
    /// An Invalid Byte Length Was Found.
    InvalidLength
}

/// A type for specifying errors when decoding Bencoded data.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct BencodeError {
    pub kind: BencodeErrorKind,
    pub desc: &'static str,
    pub pos:  Option<usize>
}

impl BencodeError {
    /// Construct a new BencodeError.
    pub fn new(kind: BencodeErrorKind, desc: &'static str, pos: Option<usize>) -> BencodeError {
        BencodeError{ kind: kind, desc: desc, pos: pos }
    }
}

impl Display for BencodeError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        try!(f.write_fmt(format_args!("Kind: {:?}", self.kind)));
        
        try!(f.write_fmt(format_args!(" Description: {}", self.desc)));
        
        if let Some(n) = self.pos {
            try!(f.write_fmt(format_args!("Position: {}", n)));
        }

        Ok(())
    }   
}

impl Error for BencodeError {
    fn description(&self) -> &str { self.desc }
    
    fn cause(&self) -> Option<&Error> { None }
}

//----------------------------------------------------------------------------//

pub type TorrentResult<T> = Result<T, TorrentError>;

/// A list specifying the types of TorrentErrors that may occur.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TorrentErrorKind {
    /// A Mandatory Key Is Missing In The File.
    MissingKey,
    /// A Value Was Found That Has The Wrong Type.
    WrongType,
    /// Some Other Error Occured.
    Other
}

/// A type for specifying errors when reading a torrent file.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TorrentError {
    kind: TorrentErrorKind,
    desc: &'static str,
    detail: Option<Cow<'static, str>>
}

impl TorrentError {
    pub fn new(kind: TorrentErrorKind, desc: &'static str) -> TorrentError {
        TorrentError{ kind: kind, desc: desc, detail: None }
    }
    
    pub fn with_detail<T>(kind: TorrentErrorKind, desc: &'static str, detail: T)
        -> TorrentError where T: Into<Cow<'static, str>> {
        TorrentError{ kind: kind, desc: desc, detail: Some(detail.into()) }
    }
    
    pub fn kind(&self) -> TorrentErrorKind {
        self.kind
    }
    
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_ref().map(|x| &**x)
    }
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

impl From<io::Error> for TorrentError {
    fn from(error: io::Error) -> TorrentError {
        TorrentError::with_detail(TorrentErrorKind::Other,
            "An io::Error Occurred, See detail",
            error.description().to_owned()
        )
    }
}

impl From<BencodeError> for TorrentError {
    fn from(error: BencodeError) -> TorrentError {
        TorrentError::with_detail(TorrentErrorKind::Other,
            "A BencodeError Occurred, See detail",
            error.to_string()
        )
    }
}

impl Error for TorrentError {
    fn description(&self) -> &str { self.desc }
    
    fn cause(&self) -> Option<&Error> { None }
}