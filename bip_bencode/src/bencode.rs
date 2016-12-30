use std::collections::BTreeMap;
use std::str;

use decode;
use dictionary::Dictionary;
use error::{BencodeParseResult, BencodeParseError, BencodeParseErrorKind};
use encode;

/// Abstract representation of a Bencode object.
pub enum BencodeKind<'b, 'a: 'b> {
    /// Bencode Integer.
    Int(i64),
    /// Bencode Bytes.
    Bytes(&'a [u8]),
    /// Bencode List.
    List(&'b [Bencode<'a>]),
    /// Bencode Dictionary.
    Dict(&'b Dictionary<'a, Bencode<'a>>),
}

/// Bencode object that holds references to the underlying data.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Bencode<'a> {
    /// Bencode Integer.
    Int(i64),
    /// Bencode Bytes.
    Bytes(&'a [u8]),
    /// Bencode List.
    List(Vec<Bencode<'a>>),
    /// Bencode Dictionary.
    Dict(BTreeMap<&'a [u8], Bencode<'a>>),
}

impl<'a> Bencode<'a> {
    pub fn decode(bytes: &'a [u8]) -> BencodeParseResult<Bencode<'a>> {
        // Apply try so any errors return before the eof check
        let (bencode, end_pos) = try!(decode::decode(bytes, 0));

        if end_pos != bytes.len() {
            return Err(BencodeParseError::from_kind(BencodeParseErrorKind::BytesEmpty{ pos: Some(end_pos) }));
        }

        Ok(bencode)
    }

    pub fn encode(&self) -> Vec<u8> {
        encode::encode(self)
    }

    pub fn kind<'b>(&'b self) -> BencodeKind<'b, 'a> {
        match self {
            &Bencode::Int(n) => BencodeKind::Int(n),
            &Bencode::Bytes(ref n) => BencodeKind::Bytes(n),
            &Bencode::List(ref n) => BencodeKind::List(n),
            &Bencode::Dict(ref n) => BencodeKind::Dict(n),
        }
    }

    pub fn str(&self) -> Option<&'a str> {
        let bytes = match self.bytes() {
            Some(n) => n,
            None => return None,
        };

        match str::from_utf8(bytes) {
            Ok(n) => Some(n),
            Err(_) => None,
        }
    }

    pub fn int(&self) -> Option<i64> {
        match self {
            &Bencode::Int(n) => Some(n),
            _ => None,
        }
    }

    pub fn bytes(&self) -> Option<&'a [u8]> {
        match self {
            &Bencode::Bytes(ref n) => Some(&n[0..]),
            _ => None,
        }
    }

    pub fn list(&self) -> Option<&[Bencode<'a>]> {
        match self {
            &Bencode::List(ref n) => Some(n),
            _ => None,
        }
    }

    pub fn dict(&self) -> Option<&Dictionary<'a, Bencode<'a>>> {
        match self {
            &Bencode::Dict(ref n) => Some(n),
            _ => None,
        }
    }
}
