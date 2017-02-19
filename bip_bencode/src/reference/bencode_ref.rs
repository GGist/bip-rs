use std::collections::BTreeMap;
use std::str;

use access::bencode::{BRefAccess, BencodeRefKind};
use reference::decode;
use reference::decode_opt::BDecodeOpt;
use access::dict::BDictAccess;
use access::list::BListAccess;
use error::{BencodeParseResult, BencodeParseError, BencodeParseErrorKind};

/// Bencode object that holds references to the underlying data.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum InnerBencodeRef<'a> {
    /// Bencode Integer.
    Int(i64, &'a [u8]),
    /// Bencode Bytes.
    Bytes(&'a [u8], &'a [u8]),
    /// Bencode List.
    List(Vec<BencodeRef<'a>>, &'a [u8]),
    /// Bencode Dictionary.
    Dict(BTreeMap<&'a [u8], BencodeRef<'a>>, &'a [u8]),
}

impl<'a> Into<BencodeRef<'a>> for InnerBencodeRef<'a> {
    fn into(self) -> BencodeRef<'a> {
        BencodeRef{ inner: self }
    }
}

/// Bencode object that holds references to the underlying data.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct BencodeRef<'a> {
    inner: InnerBencodeRef<'a>
}

impl<'a> BencodeRef<'a> {
    pub fn decode(bytes: &'a [u8], opts: BDecodeOpt) -> BencodeParseResult<BencodeRef<'a>> {
        // Apply try so any errors return before the eof check
        let (bencode, end_pos) = try!(decode::decode(bytes, 0, opts, 0));

        if end_pos != bytes.len() {
            return Err(BencodeParseError::from_kind(BencodeParseErrorKind::BytesEmpty{ pos: end_pos }));
        }

        Ok(bencode)
    }

    pub fn buffer(&self) -> &'a [u8] {
        match self.inner {
            InnerBencodeRef::Int(_, buffer)   => buffer,
            InnerBencodeRef::Bytes(_, buffer) => buffer,
            InnerBencodeRef::List(_, buffer)  => buffer,
            InnerBencodeRef::Dict(_, buffer)  => buffer
        }
    }
}

impl<'a> BRefAccess<'a> for BencodeRef<'a> {
    type BType = BencodeRef<'a>;

    fn kind<'b>(&'b self) -> BencodeRefKind<'b, 'a, BencodeRef<'a>> {
        match self.inner {
            InnerBencodeRef::Int(n, _)       => BencodeRefKind::Int(n),
            InnerBencodeRef::Bytes(ref n, _) => BencodeRefKind::Bytes(n),
            InnerBencodeRef::List(ref n, _)  => BencodeRefKind::List(n),
            InnerBencodeRef::Dict(ref n, _)  => BencodeRefKind::Dict(n),
        }
    }

    fn str(&self) -> Option<&'a str> {
        let bytes = match self.bytes() {
            Some(n) => n,
            None => return None,
        };

        match str::from_utf8(bytes) {
            Ok(n) => Some(n),
            Err(_) => None,
        }
    }

    fn int(&self) -> Option<i64> {
        match self.inner {
            InnerBencodeRef::Int(n, _) => Some(n),
            _ => None,
        }
    }

    fn bytes(&self) -> Option<&'a [u8]> {
        match self.inner {
            InnerBencodeRef::Bytes(ref n, _) => Some(&n[0..]),
            _ => None,
        }
    }

    fn list(&self) -> Option<&BListAccess<BencodeRef<'a>>> {
        match self.inner {
            InnerBencodeRef::List(ref n, _) => Some(n),
            _ => None,
        }
    }

    fn dict(&self) -> Option<&BDictAccess<'a, BencodeRef<'a>>> {
        match self.inner {
            InnerBencodeRef::Dict(ref n, _) => Some(n),
            _ => None,
        }
    }
}