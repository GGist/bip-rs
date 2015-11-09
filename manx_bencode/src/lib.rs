//! Bencode parsing and validation.

use std::collections::{BTreeMap};
use std::convert::{AsRef};
use std::{str};

use error::{BencodeResult, BencodeErrorKind, BencodeError};
use util::dictionary::{Dictionary};

mod bencode;
mod convert;
mod decode;
mod encode;
mod error;

const BEN_END:    u8 = b'e';
const DICT_START: u8 = b'd';
const LIST_START: u8 = b'l';
const INT_START:  u8 = b'i';

const BYTE_LEN_LOW:  u8 = b'0';
const BYTE_LEN_HIGH: u8 = b'9';
const BYTE_LEN_END:  u8 = b':';

/// Represents an abstraction into the contents of a BencodeView.
pub enum BencodeKind<'b, 'a: 'b> {
    /// Bencode Integer.
    Int(i64),
    /// Bencode Bytes.
    Bytes(&'a [u8]),
    /// Bencode List.
    List(&'b [Bencode<'a>]),
    /// Bencode Dictionary.
    Dict(&'b Dictionary<'a, Bencode<'a>>)
}

/// Ahead of time parser for decoding bencode.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Bencode<'a> {
    /// Bencode Integer.
    Int(i64),
    /// Bencode Bytes.
    Bytes(&'a [u8]),
    /// Bencode List.
    List(Vec<Bencode<'a>>),
    /// Bencode Dictionary.
    Dict(BTreeMap<&'a str, Bencode<'a>>)
}

impl<'a> Bencode<'a> {
    pub fn decode(bytes: &'a [u8]) -> BencodeResult<Bencode<'a>> {
        // Apply try so any errors return before the eof check
        let (bencode, end_pos) = try!(decode::decode(bytes, 0));
        
        if end_pos != bytes.len() {
            return Err(BencodeError::with_pos(BencodeErrorKind::BytesEmpty,
                "Some Bytes Were Left Over After Parsing Bencode", Some(end_pos)))
        }
        
        Ok(bencode)
    }
    
    pub fn encode(&self) -> Vec<u8> {
        encode::encode(self)
    }
    
    pub fn kind<'b>(&'b self) -> BencodeKind<'b, 'a> {
        match self {
            &Bencode::Int(n)       => BencodeKind::Int(n),
            &Bencode::Bytes(ref n) => BencodeKind::Bytes(n),
            &Bencode::List(ref n)  => BencodeKind::List(n),
            &Bencode::Dict(ref n)  => BencodeKind::Dict(n)
        }
    }
    
    pub fn str(&self) -> Option<&'a str> {
        let bytes = match self.bytes() {
            Some(n) => n,
            None    => return None
        };
    
        match str::from_utf8(bytes) {
            Ok(n)  => Some(n),
            Err(_) => None
        }
    }
    
    pub fn int(&self) -> Option<i64> {
        match self {
            &Bencode::Int(n) => Some(n),
            _                => None
        }
    }
    
    pub fn bytes(&self) -> Option<&'a [u8]> {
        match self {
            &Bencode::Bytes(ref n) => Some(&n[0..]),
            _                      => None
        }
    }
    
    pub fn list(&self) -> Option<&[Bencode<'a>]> {
    match self {
            &Bencode::List(ref n) => Some(n),
            _                     => None
        }
    }

    pub fn dict(&self) -> Option<&Dictionary<'a, Bencode<'a>>> {
        match self {
            &Bencode::Dict(ref n) => Some(n),
            _                     => None
        }
    }
}

mod macros {
    /// Construct a Bencode map by supplying a String keys and Bencode values.
    #[macro_export]
    macro_rules! ben_map {
        ( $($key:expr => $val:expr),* ) => {
            {
                use std::convert::{AsRef};
                use std::collections::{BTreeMap};
                use redox::bencode::{Bencode};
                
                let mut map = BTreeMap::new();
                $(
                    map.insert(AsRef::as_ref($key), $val);
                )*
                Bencode::Dict(map)
            }
        }
    }
    
    /// Construct a Bencode list by supplying a list of Bencode values.
    #[macro_export]
    macro_rules! ben_list {
        ( $($ben:expr),* ) => {
            {
                use redox::bencode::{Bencode};
                
                let mut list = Vec::new();
                $(
                    list.push($ben);
                )*
                Bencode::List(list)
            }
        }
    }
    
    /// Construct Bencode bytes by supplying a type convertible to Vec\<u8\>.
    #[macro_export]
    macro_rules! ben_bytes {
        ( $ben:expr ) => {
            {
                use std::convert::{AsRef};
                use redox::bencode::{Bencode};
                
                Bencode::Bytes(AsRef::as_ref($ben))
            }
        }
    }
    
    /// Construct a Bencode integer by supplying an i64.
    #[macro_export]
    macro_rules! ben_int {
        ( $ben:expr ) => {
            {
                use redox::bencode::{Bencode};
                
                Bencode::Int($ben)
            }
        }
    }
}