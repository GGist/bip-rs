use std::collections::{BTreeMap};
use std::collections::btree_map::{Entry};
use std::convert::{AsRef};
use std::str::{self};

use bencode::{self, BencodeView, BencodeKind, DecodeBencode};
use error::{BencodeError, BencodeErrorKind, BencodeResult};
use util::{Dictionary};

/// Ahead of time parser for decoding bencode.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Bencode<'a> {
    #[doc(hidden)]
    /// Bencode Integer.
    Int(i64),
    #[doc(hidden)]
    /// Bencode Bytes.
    Bytes(&'a [u8]),
    #[doc(hidden)]
    /// Bencode List.
    List(Vec<Bencode<'a>>),
    #[doc(hidden)]
    /// Bencode Dictionary.
    Dict(BTreeMap<&'a str, Bencode<'a>>)
}

impl<'a, T: ?Sized> DecodeBencode<&'a T> for Bencode<'a> where T: AsRef<[u8]> {
    fn decode(bytes: &'a T) -> BencodeResult<Bencode<'a>> {
        let bytes_ref = bytes.as_ref();

        // Apply try so any errors return before the eof check
        let (bencode, end_pos) = try!(decode(bytes_ref, 0));
        
        if end_pos != bytes_ref.len() {
            return Err(BencodeError::with_pos(BencodeErrorKind::BytesEmpty,
                "Some Bytes Were Left Over After Parsing Bencode", Some(end_pos)))
        }
        
        Ok(bencode)
    }
}

impl<'a> BencodeView<'a> for Bencode<'a> {
    type InnerView = Bencode<'a>;

    fn kind<'b>(&'b self) -> BencodeKind<'b, 'a, Self::InnerView> {
        match self {
            &Bencode::Int(n)       => BencodeKind::Int(n),
            &Bencode::Bytes(ref n) => BencodeKind::Bytes(n),
            &Bencode::List(ref n)  => BencodeKind::List(n),
            &Bencode::Dict(ref n)  => BencodeKind::Dict(n)
        }
   }
   
    fn int(&self) -> Option<i64> {
        match self {
            &Bencode::Int(n) => Some(n),
            _                => None
        }
    }
    
    fn bytes(&self) -> Option<&'a [u8]> {
        match self {
            &Bencode::Bytes(ref n) => Some(&n[0..]),
            _                      => None
        }
    }
    
    fn list(&self) -> Option<&[Self::InnerView]> {
    match self {
            &Bencode::List(ref n) => Some(n),
            _                     => None
        }
    }

    fn dict(&self) -> Option<&Dictionary<'a, Self::InnerView>> {
        match self {
            &Bencode::Dict(ref n) => Some(n),
            _                     => None
        }
    }
}

pub fn decode<'a>(bytes: &'a [u8], pos: usize) -> BencodeResult<(Bencode<'a>, usize)> {
    let curr_byte = try!(peek_byte(bytes, pos, "End Of Bytes Encountered"));
    
    match curr_byte {
        bencode::INT_START  => {
            let (bencode, pos) = try!(decode_int(bytes, pos + 1, bencode::BEN_END));
            Ok((Bencode::Int(bencode), pos))
        },
        bencode::LIST_START => {
            let (bencode, pos) = try!(decode_list(bytes, pos + 1));
            Ok((Bencode::List(bencode), pos))
        },
        bencode::DICT_START => {
            let (bencode, pos) = try!(decode_dict(bytes, pos + 1));
            Ok((Bencode::Dict(bencode), pos))
        },
        bencode::BYTE_LEN_LOW...bencode::BYTE_LEN_HIGH => {
            let (bencode, pos) = try!(decode_bytes(bytes, pos));
            // Include the length digit, don't increment position
            Ok((Bencode::Bytes(bencode), pos))
        },
        _ => Err(BencodeError::with_pos(BencodeErrorKind::InvalidByte, 
                 "Unknown Bencode Type Token Found", Some(pos)))
    }
}

fn decode_int(bytes: &[u8], pos: usize, delim: u8) -> BencodeResult<(i64, usize)> {
    let (_, begin_decode) = bytes.split_at(pos);
    
    let relative_end_pos = match begin_decode.iter().position(|n| *n == delim) {
        Some(end_pos) => end_pos,
        None          => return Err(BencodeError::with_pos(BencodeErrorKind::InvalidInt,
                             "No Delimiter Found For Integer/Length", Some(pos)))
    };
    let int_byte_slice = &begin_decode[..relative_end_pos];
    
    if int_byte_slice.len() > 1 {
        // Negative zero is not allowed (this would not be caught when converting)
        if int_byte_slice[0] == b'-' && int_byte_slice[1] == b'0' {
            return Err(BencodeError::with_pos(BencodeErrorKind::InvalidInt,
                "Illegal Negative Zero For Integer/Length", Some(pos)))
        }
    
        // Zero padding is illegal, and unspecified for key lengths (we disallow both)
        if int_byte_slice[0] == b'0' {
            return Err(BencodeError::with_pos(BencodeErrorKind::InvalidInt,
                "Illegal Zero Padding For Integer/Length", Some(pos)))
        }
    }
    
    let int_str = match str::from_utf8(int_byte_slice) {
        Ok(n)  => n,
        Err(_) => return Err(BencodeError::with_pos(BencodeErrorKind::InvalidInt,
                      "Invalid UTF-8 Found For Integer/Length", Some(pos)))
    };
    
    // Position of end of integer type, next byte is the start of the next value
    let absolute_end_pos = pos + relative_end_pos;
    match i64::from_str_radix(int_str, 10) {
        Ok(n)  => Ok((n, absolute_end_pos + 1)),
        Err(_) => Err(BencodeError::with_pos(BencodeErrorKind::InvalidInt,
                      "Could Not Convert Integer/Length To i64", Some(pos)))
    }
}
    
fn decode_bytes<'a>(bytes: &'a [u8], pos: usize) -> BencodeResult<(&'a [u8], usize)> {
    let (num_bytes, start_pos) = try!(decode_int(bytes, pos, bencode::BYTE_LEN_END));

    if num_bytes < 0 {
        return Err(BencodeError::with_pos(BencodeErrorKind::InvalidLength, 
            "Negative Byte Length Found", Some(pos)))
    } 
    
    // Should be safe to cast to usize (TODO: Check if cast would overflow to provide
    // a more helpful error message, otherwise, parsing will probably fail with an
    // unrelated message).
    let num_bytes = num_bytes as usize;
    
    if num_bytes > bytes[start_pos..].len() {
        return Err(BencodeError::with_pos(BencodeErrorKind::InvalidLength,
            "Overflow Byte Length Found", Some(pos)))
    }
    
    let end_pos = start_pos + num_bytes;
    Ok((&bytes[start_pos..end_pos], end_pos))
}

fn decode_list<'a>(bytes: &'a [u8], pos: usize) -> BencodeResult<(Vec<Bencode<'a>>, usize)> {
    let mut bencode_list = Vec::new();
    
    let mut curr_pos = pos;
    let mut curr_byte = try!(peek_byte(bytes, curr_pos, "End Of Bytes Element Encountered In List"));
    
    while curr_byte != bencode::BEN_END {
        let (bencode, next_pos) = try!(decode(bytes, curr_pos));
        
        bencode_list.push(bencode);
        
        curr_pos = next_pos;
        curr_byte = try!(peek_byte(bytes, curr_pos, "End Of Bytes Element Encountered In List"));
    }
    
    Ok((bencode_list, curr_pos + 1))
}

fn decode_dict<'a>(bytes: &'a [u8], pos: usize) -> BencodeResult<(BTreeMap<&'a str, Bencode<'a>>, usize)> {
    let mut bencode_dict = BTreeMap::new();
    
    let mut curr_pos = pos;
    let mut curr_byte = try!(peek_byte(bytes, curr_pos, "End Of Bytes Element Encountered In Dictionary"));
    
    while curr_byte != bencode::BEN_END {
        let (key_bytes, next_pos) = try!(decode_bytes(bytes, curr_pos));
    
        let key = match str::from_utf8(key_bytes) {
            Ok(n)  => n,
            Err(_) => {
                return Err(BencodeError::with_pos(BencodeErrorKind::InvalidByte,
                    "Invalid UTF-8 Key Found For Dictionar", Some(curr_pos)))
            }
        };
        
        // Spec says that the keys must be in alphabetical order
        match bencode_dict.keys().last() {
            Some(last_key) if key < *last_key => {
                return Err(BencodeError::with_pos(BencodeErrorKind::InvalidKey,
                "Key Not In Alphabetical Order For Dictionary", Some(curr_pos)))
            },
            _ => ()
        };
        curr_pos = next_pos;
        
        let (value, next_pos) = try!(decode(bytes, curr_pos));
        match bencode_dict.entry(key) {
            Entry::Vacant(n)   => n.insert(value),
            Entry::Occupied(_) => {
                return Err(BencodeError::with_pos(BencodeErrorKind::InvalidKey,
                    "Duplicate Key Found For Dictionary", Some(curr_pos)))
            }
        };

        curr_pos = next_pos;
        curr_byte = try!(peek_byte(bytes, curr_pos, "End Of Bytes Element Encountered In Dictionary"));
    }
    
    Ok((bencode_dict, curr_pos + 1))
}

fn peek_byte(bytes: &[u8], pos: usize, err_msg: &'static str) -> BencodeResult<u8> {
    bytes.get(pos).map(|n| *n).ok_or( BencodeError::new(BencodeErrorKind::BytesEmpty, err_msg) )
}

#[cfg(test)]
mod tests {
    use super::{Bencode};
    use bencode::{self, BencodeView, DecodeBencode};

    // Positive Cases
    const GENERAL: &'static [u8] = b"d0:12:zero_len_key8:location17:udp://test.com:8011:nested dictd4:listli-500500eee6:numberi500500ee";
    const BYTES_UTF8: &'static [u8] = b"16:valid_utf8_bytes";
    const DICTIONARY: &'static [u8] = b"d9:test_dictd10:nested_key12:nested_value11:nested_listli500ei-500ei0eee8:test_key10:test_valuee";
    const LIST: &'static [u8] = b"l10:test_bytesi500ei0ei-500el12:nested_bytesed8:test_key10:test_valueee";
    const BYTES: &'static [u8] = b"5:\xC5\xE6\xBE\xE6\xF2";
    const BYTES_ZERO_LEN: &'static [u8] = b"0:";
    const INT: &'static [u8] = b"i500e";
    const INT_NEGATIVE: &'static [u8] = b"i-500e";
    const INT_ZERO: &'static [u8] = b"i0e";
   
    // Negative Cases
    const BYTES_NEG_LEN: &'static [u8] = b"-4:test";
    const BYTES_EXTRA: &'static [u8] = b"l15:processed_bytese17:unprocessed_bytes";
    const BYTES_NOT_UTF8: &'static [u8] = b"5:\xC5\xE6\xBE\xE6\xF2";
    const INT_NAN: &'static [u8] = b"i500a500e";
    const INT_LEADING_ZERO: &'static [u8] = b"i0500e";
    const INT_DOUBLE_ZERO: &'static [u8] = b"i00e";
    const INT_NEGATIVE_ZERO: &'static [u8] = b"i-0e";
    const INT_DOUBLE_NEGATIVE: &'static [u8] = b"i--5e";
    const DICT_UNORDERED_KEYS: &'static [u8] = b"d5:z_key5:value5:a_key5:valuee";
    const DICT_DUP_KEYS_SAME_DATA: &'static [u8] = b"d5:a_keyi0e5:a_keyi0ee";
    const DICT_DUP_KEYS_DIFF_DATA: &'static [u8] = b"d5:a_keyi0e5:a_key7:a_valuee";
   
   #[test]
   fn positive_decode_general() {
        let bencode = Bencode::decode(GENERAL).unwrap();
        
        let ben_dict = bencode.dict().unwrap();
        assert_eq!(ben_dict.lookup("").unwrap().str().unwrap(), "zero_len_key");
        assert_eq!(ben_dict.lookup("location").unwrap().str().unwrap(), "udp://test.com:80");
        assert_eq!(ben_dict.lookup("number").unwrap().int().unwrap(), 500500i64);
        
        let nested_dict = ben_dict.lookup("nested dict").unwrap().dict().unwrap();
        let nested_list = nested_dict.lookup("list").unwrap().list().unwrap();
        assert_eq!(nested_list[0].int().unwrap(), -500500i64);
   }
    
   #[test]
   fn positive_decode_bytes_utf8() {
        let bencode = Bencode::decode(BYTES_UTF8).unwrap();
        
        assert_eq!(bencode.str().unwrap(), "valid_utf8_bytes");
   }
   
    #[test]
    fn positive_decode_dict() {
        let dict = super::decode_dict(DICTIONARY, 1).unwrap().0;
        assert_eq!(dict.get("test_key").unwrap().str().unwrap(), "test_value");
        
        let nested_dict = dict.get("test_dict").unwrap().dict().unwrap();
        assert_eq!(nested_dict.lookup("nested_key").unwrap().str().unwrap(), "nested_value");
        
        let nested_list = nested_dict.lookup("nested_list").unwrap().list().unwrap();
        assert_eq!(nested_list[0].int().unwrap(), 500i64);
        assert_eq!(nested_list[1].int().unwrap(), -500i64);
        assert_eq!(nested_list[2].int().unwrap(), 0i64);
    }
   
    #[test]
    fn positive_decode_list() {
        let list = super::decode_list(LIST, 1).unwrap().0;
        assert_eq!(list[0].str().unwrap(), "test_bytes");
        assert_eq!(list[1].int().unwrap(), 500i64);
        assert_eq!(list[2].int().unwrap(), 0i64);
        assert_eq!(list[3].int().unwrap(), -500i64);
        
        let nested_list = list[4].list().unwrap();
        assert_eq!(nested_list[0].str().unwrap(), "nested_bytes");
        
        let nested_dict = list[5].dict().unwrap();
        assert_eq!(nested_dict.lookup("test_key").unwrap().str().unwrap(), "test_value");
    }
   
    #[test]
    fn positive_decode_bytes() {
        let bytes = super::decode_bytes(BYTES, 0).unwrap().0;
        assert_eq!(bytes.len(), 5);
        assert_eq!(bytes[0] as char, 'Å');
        assert_eq!(bytes[1] as char, 'æ');
        assert_eq!(bytes[2] as char, '¾');
        assert_eq!(bytes[3] as char, 'æ');
        assert_eq!(bytes[4] as char, 'ò');
    }
    
    #[test]
    fn positive_decode_bytes_zero_len() {
        let bytes = super::decode_bytes(BYTES_ZERO_LEN, 0).unwrap().0;
        assert_eq!(bytes.len(), 0);
    }
   
    #[test]
    fn positive_decode_int() {
        let int_value = super::decode_int(INT, 1, bencode::BEN_END).unwrap().0;
        assert_eq!(int_value, 500i64);
    }
   
    #[test]
    fn positive_decode_int_negative() {
        let int_value = super::decode_int(INT_NEGATIVE, 1, bencode::BEN_END).unwrap().0;
        assert_eq!(int_value, -500i64);
    }
    
    #[test]
    fn positive_decode_int_zero() {
        let int_value = super::decode_int(INT_ZERO, 1, bencode::BEN_END).unwrap().0;
        assert_eq!(int_value, 0i64);
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_bytes_neg_len() {
        Bencode::decode(BYTES_NEG_LEN).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_bytes_extra() {
        Bencode::decode(BYTES_EXTRA).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_bytes_not_utf8() {
        let bencode = Bencode::decode(BYTES_NOT_UTF8).unwrap();
        
        bencode.str().unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_nan() {
        super::decode_int(INT_NAN, 1, bencode::BEN_END).unwrap().0;
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_leading_zero() {
        super::decode_int(INT_LEADING_ZERO, 1, bencode::BEN_END).unwrap().0;
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_double_zero() {
        super::decode_int(INT_DOUBLE_ZERO, 1, bencode::BEN_END).unwrap().0;
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_negative_zero() {
        super::decode_int(INT_NEGATIVE_ZERO, 1, bencode::BEN_END).unwrap().0;
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_double_negative() {
        super::decode_int(INT_DOUBLE_NEGATIVE, 1, bencode::BEN_END).unwrap().0;
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_dict_unordered_keys() {
        super::decode_dict(DICT_UNORDERED_KEYS, 1).unwrap().0;
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_dict_dup_keys_same_data() {
        super::decode_dict(DICT_DUP_KEYS_SAME_DATA, 1).unwrap().0;
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_dict_dup_keys_diff_data() {
        super::decode_dict(DICT_DUP_KEYS_DIFF_DATA, 1).unwrap().0;
    }
}