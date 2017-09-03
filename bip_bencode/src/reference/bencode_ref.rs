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

/// `BencodeRef` object that stores references to some buffer.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct BencodeRef<'a> {
    inner: InnerBencodeRef<'a>
}

impl<'a> BencodeRef<'a> {
    /// Decode the given bytes into a `BencodeRef` using the given decode options.
    pub fn decode(bytes: &'a [u8], opts: BDecodeOpt) -> BencodeParseResult<BencodeRef<'a>> {
        // Apply try so any errors return before the eof check
        let (bencode, end_pos) = try!(decode::decode(bytes, 0, opts, 0));

        if end_pos != bytes.len() && opts.enforce_full_decode() {
            return Err(BencodeParseError::from_kind(BencodeParseErrorKind::BytesEmpty{ pos: end_pos }));
        }

        Ok(bencode)
    }

    /// Get a byte slice of the current bencode byte representation.
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

#[cfg(test)]
mod tests {
    use std::default::Default;

    use access::bencode::BRefAccess;
    use reference::bencode_ref::BencodeRef;
    use reference::decode_opt::BDecodeOpt;

    #[test]
    fn positive_int_buffer() {
        let int_bytes = b"i-500e";
        let bencode = BencodeRef::decode(&int_bytes[..], BDecodeOpt::default()).unwrap();

        assert_eq!(int_bytes, bencode.buffer());
    }

    #[test]
    fn positive_bytes_buffer() {
        let bytes_bytes = b"3:asd";
        let bencode = BencodeRef::decode(&bytes_bytes[..], BDecodeOpt::default()).unwrap();

        assert_eq!(bytes_bytes, bencode.buffer());
    }

    #[test]
    fn positive_list_buffer() {
        let list_bytes = b"l3:asde";
        let bencode = BencodeRef::decode(&list_bytes[..], BDecodeOpt::default()).unwrap();

        assert_eq!(list_bytes, bencode.buffer());
    }

    #[test]
    fn positive_dict_buffer() {
        let dict_bytes = b"d3:asd3:asde";
        let bencode = BencodeRef::decode(&dict_bytes[..], BDecodeOpt::default()).unwrap();

        assert_eq!(dict_bytes, bencode.buffer());
    }

    #[test]
    fn positive_list_nested_int_buffer() {
        let nested_int_bytes = b"li-500ee";
        let bencode = BencodeRef::decode(&nested_int_bytes[..], BDecodeOpt::default()).unwrap();

        let bencode_list = bencode.list().unwrap();
        let bencode_int = bencode_list.get(0).unwrap();

        let int_bytes = b"i-500e";
        assert_eq!(int_bytes, bencode_int.buffer());
    }

    #[test]
    fn positive_dict_nested_int_buffer() {
        let nested_int_bytes = b"d3:asdi-500ee";
        let bencode = BencodeRef::decode(&nested_int_bytes[..], BDecodeOpt::default()).unwrap();

        let bencode_dict = bencode.dict().unwrap();
        let bencode_int = bencode_dict.lookup(&b"asd"[..]).unwrap();

        let int_bytes = b"i-500e";
        assert_eq!(int_bytes, bencode_int.buffer());
    }

    #[test]
    fn positive_list_nested_bytes_buffer() {
        let nested_bytes_bytes = b"l3:asde";
        let bencode = BencodeRef::decode(&nested_bytes_bytes[..], BDecodeOpt::default()).unwrap();

        let bencode_list = bencode.list().unwrap();
        let bencode_bytes = bencode_list.get(0).unwrap();

        let bytes_bytes = b"3:asd";
        assert_eq!(bytes_bytes, bencode_bytes.buffer());
    }

    #[test]
    fn positive_dict_nested_bytes_buffer() {
        let nested_bytes_bytes = b"d3:asd3:asde";
        let bencode = BencodeRef::decode(&nested_bytes_bytes[..], BDecodeOpt::default()).unwrap();

        let bencode_dict = bencode.dict().unwrap();
        let bencode_bytes = bencode_dict.lookup(&b"asd"[..]).unwrap();

        let bytes_bytes = b"3:asd";
        assert_eq!(bytes_bytes, bencode_bytes.buffer());
    }

    #[test]
    fn positive_list_nested_list_buffer() {
        let nested_list_bytes = b"ll3:asdee";
        let bencode = BencodeRef::decode(&nested_list_bytes[..], BDecodeOpt::default()).unwrap();

        let bencode_list = bencode.list().unwrap();
        let bencode_list = bencode_list.get(0).unwrap();

        let list_bytes = b"l3:asde";
        assert_eq!(list_bytes, bencode_list.buffer());
    }

    #[test]
    fn positive_dict_nested_list_buffer() {
        let nested_list_bytes = b"d3:asdl3:asdee";
        let bencode = BencodeRef::decode(&nested_list_bytes[..], BDecodeOpt::default()).unwrap();

        let bencode_dict = bencode.dict().unwrap();
        let bencode_list = bencode_dict.lookup(&b"asd"[..]).unwrap();

        let list_bytes = b"l3:asde";
        assert_eq!(list_bytes, bencode_list.buffer());
    }

    #[test]
    fn positive_list_nested_dict_buffer() {
        let nested_dict_bytes = b"ld3:asd3:asdee";
        let bencode = BencodeRef::decode(&nested_dict_bytes[..], BDecodeOpt::default()).unwrap();

        let bencode_list = bencode.list().unwrap();
        let bencode_dict = bencode_list.get(0).unwrap();

        let dict_bytes = b"d3:asd3:asde";
        assert_eq!(dict_bytes, bencode_dict.buffer());
    }

    #[test]
    fn positive_dict_nested_dict_buffer() {
        let nested_dict_bytes = b"d3:asdd3:asd3:asdee";
        let bencode = BencodeRef::decode(&nested_dict_bytes[..], BDecodeOpt::default()).unwrap();

        let bencode_dict = bencode.dict().unwrap();
        let bencode_dict = bencode_dict.lookup(&b"asd"[..]).unwrap();

        let dict_bytes = b"d3:asd3:asde";
        assert_eq!(dict_bytes, bencode_dict.buffer());
    }
}