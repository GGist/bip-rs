use std::borrow::{Cow};
use std::error::{Error};
use std::fmt::{self, Display, Formatter};

//----------------------------------------------------------------------------//

/// Result of parsing bencoded data.
pub type BencodeParseResult<T> = Result<T, BencodeParseError>;

/// Enumerates all bencode parse errors.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BencodeParseErrorKind {
    /// An Incomplete Number Of Bytes.
    BytesEmpty,
    /// An Invalid Byte Was Found.
    InvalidByte,
    /// An Invalid Integer Was Found.
    InvalidInt,
    /// An Invalid Key Was Found.
    InvalidKey,
    /// An Invalid Byte Length Was Found.
    InvalidLength
}

/// Error type generated when parsing bencoded data.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct BencodeParseError {
    kind: BencodeParseErrorKind,
    desc: &'static str,
    pos:  Option<usize>
}

impl BencodeParseError {
    pub fn new(kind: BencodeParseErrorKind, desc: &'static str) -> BencodeParseError {
        BencodeParseError::with_pos(kind, desc, None)
    }
    
    pub fn with_pos(kind: BencodeParseErrorKind, desc: &'static str, pos: Option<usize>) -> BencodeParseError {
        BencodeParseError{ kind: kind, desc: desc, pos: pos }
    }
    
    pub fn kind(&self) -> BencodeParseErrorKind {
        self.kind
    }
    
    pub fn position(&self) -> Option<usize> {
        self.pos
    }
}

impl Display for BencodeParseError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        try!(f.write_fmt(format_args!("Kind: {:?}", self.kind)));
        
        try!(f.write_fmt(format_args!(", Description: {}", self.desc)));
        
        if let Some(n) = self.pos {
            try!(f.write_fmt(format_args!(", Position: {}", n)));
        }

        Ok(())
    }   
}

impl Error for BencodeParseError {
    fn description(&self) -> &str { self.desc }
    
    fn cause(&self) -> Option<&Error> { None }
}

//----------------------------------------------------------------------------//

/// Result of converting a bencode object.
pub type BencodeConvertResult<T> = Result<T, BencodeConvertError>;

/// Enumerates all bencode conversion errors.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BencodeConvertErrorKind {
    /// A key is missing in the bencode dictionary.
    MissingKey,
    /// A bencode value has the wrong type.
    WrongType
}

/// Error type generated when converting bencode objects.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct BencodeConvertError {
    kind: BencodeConvertErrorKind,
    desc: &'static str,
    key: Cow<'static, str>
}

impl BencodeConvertError {
    pub fn new(kind: BencodeConvertErrorKind, desc: &'static str) -> BencodeConvertError {
        BencodeConvertError::with_key(kind, desc, "")
    }

    pub fn with_key<T>(kind: BencodeConvertErrorKind, desc: &'static str, key: T)
        -> BencodeConvertError where T: Into<Cow<'static, str>> {
        BencodeConvertError{ kind: kind, desc: desc, key: key.into() }
    }
    
    pub fn kind(&self) -> BencodeConvertErrorKind {
        self.kind
    }
    
    pub fn desc(&self) -> &'static str {
        self.desc
    }
    
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Display for BencodeConvertError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        try!(f.write_fmt(format_args!("Kind: {:?}", self.kind)));
        
        try!(f.write_fmt(format_args!(", Description: {}", self.desc)));
        
        try!(f.write_fmt(format_args!(", Key: {}", self.key)));
        
        Ok(())
    }   
}

impl Error for BencodeConvertError {
    fn description(&self) -> &str { self.desc }
    
    fn cause(&self) -> Option<&Error> { None }
}