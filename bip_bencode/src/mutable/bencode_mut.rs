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

/// `BencodeMut` object that stores references to some data.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct BencodeMut<'a> {
    inner: InnerBencodeMut<'a>
}

impl<'a> BencodeMut<'a> {
    fn new(inner: InnerBencodeMut<'a>) -> BencodeMut<'a> {
        BencodeMut{ inner: inner }
    }

    /// Create a new `BencodeMut` representing an `i64`.
    pub fn new_int(value: i64) -> BencodeMut<'a> {
        BencodeMut::new(InnerBencodeMut::Int(value))
    }

    /// Create a new `BencodeMut` representing a `[u8]`.
    pub fn new_bytes(value: &'a [u8]) -> BencodeMut<'a> {
        BencodeMut::new(InnerBencodeMut::Bytes(value))
    }

    /// Create a new `BencodeMut` representing a `BListAccess`.
    pub fn new_list() -> BencodeMut<'a> {
        BencodeMut::new(InnerBencodeMut::List(Vec::new()))
    }

    /// Create a new `BencodeMut` representing a `BDictAccess`.
    pub fn new_dict() -> BencodeMut<'a> {
        BencodeMut::new(InnerBencodeMut::Dict(BTreeMap::new()))
    }

    /// Encode the `BencodeMut` into a buffer representing the bencode.
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

#[cfg(test)]
mod test {
    use access::bencode::BMutAccess;
    use mutable::bencode_mut::BencodeMut;

    #[test]
    fn positive_int_encode() {
        let bencode_int = BencodeMut::new_int(-560);

        let int_bytes = b"i-560e";
        assert_eq!(&int_bytes[..], &bencode_int.encode()[..]);
    }

    #[test]
    fn positive_bytes_encode() {
        let bencode_bytes = BencodeMut::new_bytes(b"asdasd");

        let bytes_bytes = b"6:asdasd";
        assert_eq!(&bytes_bytes[..], &bencode_bytes.encode()[..]);
    }

    #[test]
    fn positive_empty_list_encode() {
        let bencode_list = BencodeMut::new_list();

        let list_bytes = b"le";
        assert_eq!(&list_bytes[..], &bencode_list.encode()[..]);
    }

    #[test]
    fn positive_nonempty_list_encode() {
        let mut bencode_list = BencodeMut::new_list();

        {
            let list_mut = bencode_list.list_mut().unwrap();
            list_mut.push(BencodeMut::new_int(56));
        }

        let list_bytes = b"li56ee";
        assert_eq!(&list_bytes[..], &bencode_list.encode()[..]);
    }

    #[test]
    fn positive_empty_dict_encode() {
        let bencode_dict = BencodeMut::new_dict();

        let dict_bytes = b"de";
        assert_eq!(&dict_bytes[..], &bencode_dict.encode()[..]);
    }

    #[test]
    fn positive_nonempty_dict_encode() {
        let mut bencode_dict = BencodeMut::new_dict();

        {
            let dict_mut = bencode_dict.dict_mut().unwrap();
            dict_mut.insert(b"asd", BencodeMut::new_bytes(b"asdasd"));
        }

        let dict_bytes = b"d3:asd6:asdasde";
        assert_eq!(&dict_bytes[..], &bencode_dict.encode()[..]);
    }
}