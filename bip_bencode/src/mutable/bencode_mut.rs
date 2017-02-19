use std::collections::BTreeMap;
use std::str;

use access::bencode::{BencodeMutKind, BMutAccess, BRefAccess, BencodeRefKind};
use access::dict::BDictAccess;
use access::list::BListAccess;
use mutable::encode;

/// Bencode object that holds references to the underlying data.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum InnerBencodeMut<'a> {
    /// Bencode Integer.
    Int(i64),
    /// Bencode Bytes.
    Bytes(&'a [u8]),
    /// Bencode List.
    List(Vec<BencodeMut<'a>>),
    /// Bencode Dictionary.
    Dict(BTreeMap<&'a [u8], BencodeMut<'a>>),
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct BencodeMut<'a> {
    inner: InnerBencodeMut<'a>
}

impl<'a> BencodeMut<'a> {
    fn new(inner: InnerBencodeMut<'a>) -> BencodeMut<'a> {
        BencodeMut{ inner: inner }
    }

    pub fn new_int(value: i64) -> BencodeMut<'a> {
        BencodeMut::new(InnerBencodeMut::Int(value))
    }

    pub fn new_bytes(value: &'a [u8]) -> BencodeMut<'a> {
        BencodeMut::new(InnerBencodeMut::Bytes(value))
    }

    pub fn new_list() -> BencodeMut<'a> {
        BencodeMut::new(InnerBencodeMut::List(Vec::new()))
    }

    pub fn new_dict() -> BencodeMut<'a> {
        BencodeMut::new(InnerBencodeMut::Dict(BTreeMap::new()))
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        encode::encode(self, &mut buffer);

        buffer
    }
}

impl<'a> BRefAccess<'a> for BencodeMut<'a> {
    type BType = BencodeMut<'a>;

    fn kind<'b>(&'b self) -> BencodeRefKind<'b, 'a, BencodeMut<'a>> {
        match self.inner {
            InnerBencodeMut::Int(n)       => BencodeRefKind::Int(n),
            InnerBencodeMut::Bytes(ref n) => BencodeRefKind::Bytes(n),
            InnerBencodeMut::List(ref n)  => BencodeRefKind::List(n),
            InnerBencodeMut::Dict(ref n)  => BencodeRefKind::Dict(n),
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
            InnerBencodeMut::Int(n) => Some(n),
            _ => None,
        }
    }

    fn bytes(&self) -> Option<&'a [u8]> {
        match self.inner {
            InnerBencodeMut::Bytes(ref n) => Some(&n[0..]),
            _ => None,
        }
    }

    fn list(&self) -> Option<&BListAccess<BencodeMut<'a>>> {
        match self.inner {
            InnerBencodeMut::List(ref n) => Some(n),
            _ => None,
        }
    }

    fn dict(&self) -> Option<&BDictAccess<'a, BencodeMut<'a>>> {
        match self.inner {
            InnerBencodeMut::Dict(ref n) => Some(n),
            _ => None,
        }
    }
}

impl<'a> BMutAccess<'a> for BencodeMut<'a> {
    fn kind_mut<'b>(&'b mut self) -> BencodeMutKind<'b, 'a, BencodeMut<'a>> {
        match self.inner {
            InnerBencodeMut::Int(n)           => BencodeMutKind::Int(n),
            InnerBencodeMut::Bytes(ref mut n) => BencodeMutKind::Bytes(n),
            InnerBencodeMut::List(ref mut n)  => BencodeMutKind::List(n),
            InnerBencodeMut::Dict(ref mut n)  => BencodeMutKind::Dict(n),
        }
    }

    fn list_mut(&mut self) -> Option<&mut BListAccess<BencodeMut<'a>>> {
        match self.inner {
            InnerBencodeMut::List(ref mut n) => Some(n),
            _ => None
        }
    }

    fn dict_mut(&mut self) -> Option<&mut BDictAccess<'a, BencodeMut<'a>>> {
        match self.inner {
            InnerBencodeMut::Dict(ref mut n) => Some(n),
            _ => None
        }
    }
}