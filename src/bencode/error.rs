use std::error::{Error};
use std::fmt::{self, Display, Formatter};
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
    InvalidLength,
    /// Some Other Error, Possibly Converted From Another Type.
    Other
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