use std::collections::{BTreeMap};
use std::collections::btree_map::{Entry};
use std::str::{self};

use bencode::{Bencode};
use error::{BencodeParseError, BencodeParseErrorKind, BencodeParseResult};

pub fn decode<'a>(bytes: &'a [u8], pos: usize) -> BencodeParseResult<(Bencode<'a>, usize)> {
    let curr_byte = try!(peek_byte(bytes, pos));
    
    match curr_byte {
        ::INT_START  => {
            let (bencode, pos) = try!(decode_int(bytes, pos + 1, ::BEN_END));
            Ok((Bencode::Int(bencode), pos))
        },
        ::LIST_START => {
            let (bencode, pos) = try!(decode_list(bytes, pos + 1));
            Ok((Bencode::List(bencode), pos))
        },
        ::DICT_START => {
            let (bencode, pos) = try!(decode_dict(bytes, pos + 1));
            Ok((Bencode::Dict(bencode), pos))
        },
        ::BYTE_LEN_LOW...::BYTE_LEN_HIGH => {
            let (bencode, pos) = try!(decode_bytes(bytes, pos));
            // Include the length digit, don't increment position
            Ok((Bencode::Bytes(bencode), pos))
        },
        _ => Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidByte{ pos: pos }))
    }
}

fn decode_int(bytes: &[u8], pos: usize, delim: u8) -> BencodeParseResult<(i64, usize)> {
    let (_, begin_decode) = bytes.split_at(pos);
    
    let relative_end_pos = match begin_decode.iter().position(|n| *n == delim) {
        Some(end_pos) => end_pos,
        None          => return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntNoDelimiter{ pos: pos }))
    };
    let int_byte_slice = &begin_decode[..relative_end_pos];
    
    if int_byte_slice.len() > 1 {
        // Negative zero is not allowed (this would not be caught when converting)
        if int_byte_slice[0] == b'-' && int_byte_slice[1] == b'0' {
            return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntNegativeZero{ pos: pos }))
        }
    
        // Zero padding is illegal, and unspecified for key lengths (we disallow both)
        if int_byte_slice[0] == b'0' {
            return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntZeroPadding{ pos: pos }))
        }
    }
    
    let int_str = match str::from_utf8(int_byte_slice) {
        Ok(n)  => n,
        Err(_) => return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntParseError{ pos: pos }))
    };
    
    // Position of end of integer type, next byte is the start of the next value
    let absolute_end_pos = pos + relative_end_pos;
    match i64::from_str_radix(int_str, 10) {
        Ok(n)  => Ok((n, absolute_end_pos + 1)),
        Err(_) => Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntParseError{ pos: pos }))
    }
}
    
fn decode_bytes<'a>(bytes: &'a [u8], pos: usize) -> BencodeParseResult<(&'a [u8], usize)> {
    let (num_bytes, start_pos) = try!(decode_int(bytes, pos, ::BYTE_LEN_END));

    if num_bytes < 0 {
        return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidLengthNegative{ pos: pos }))
    } 
    
    // Should be safe to cast to usize (TODO: Check if cast would overflow to provide
    // a more helpful error message, otherwise, parsing will probably fail with an
    // unrelated message).
    let num_bytes = num_bytes as usize;
    
    if num_bytes > bytes[start_pos..].len() {
        return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidLengthOverflow{ pos: pos }))
    }
    
    let end_pos = start_pos + num_bytes;
    Ok((&bytes[start_pos..end_pos], end_pos))
}

fn decode_list<'a>(bytes: &'a [u8], pos: usize) -> BencodeParseResult<(Vec<Bencode<'a>>, usize)> {
    let mut bencode_list = Vec::new();
    
    let mut curr_pos = pos;
    let mut curr_byte = try!(peek_byte(bytes, curr_pos));
    
    while curr_byte != ::BEN_END {
        let (bencode, next_pos) = try!(decode(bytes, curr_pos));
        
        bencode_list.push(bencode);
        
        curr_pos = next_pos;
        curr_byte = try!(peek_byte(bytes, curr_pos));
    }
    
    Ok((bencode_list, curr_pos + 1))
}

fn decode_dict<'a>(bytes: &'a [u8], pos: usize) -> BencodeParseResult<(BTreeMap<&'a [u8], Bencode<'a>>, usize)> {
    let mut bencode_dict = BTreeMap::new();
    
    let mut curr_pos = pos;
    let mut curr_byte = try!(peek_byte(bytes, curr_pos));
    
    while curr_byte != ::BEN_END {
        let (key_bytes, next_pos) = try!(decode_bytes(bytes, curr_pos));
        
        // Spec says that the keys must be in alphabetical order
        match bencode_dict.keys().last() {
            Some(last_key) if key_bytes < *last_key => {
                return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidKeyOrdering{ pos: curr_pos, key: key_bytes.to_vec() }))
            },
            _ => ()
        };
        curr_pos = next_pos;
        
        let (value, next_pos) = try!(decode(bytes, curr_pos));
        match bencode_dict.entry(key_bytes) {
            Entry::Vacant(n)   => n.insert(value),
            Entry::Occupied(_) => {
                return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidKeyDuplicates{ pos: curr_pos, key: key_bytes.to_vec() }))
            }
        };

        curr_pos = next_pos;
        curr_byte = try!(peek_byte(bytes, curr_pos));
    }
    
    Ok((bencode_dict, curr_pos + 1))
}

fn peek_byte(bytes: &[u8], pos: usize) -> BencodeParseResult<u8> {
    bytes.get(pos)
        .map(|n| *n)
        .ok_or_else(|| BencodeParseError::from_kind(BencodeParseErrorKind::BytesEmpty{ pos: pos }))
}

#[cfg(test)]
mod tests {
    use bencode::Bencode;

    // Positive Cases
    const GENERAL: &'static [u8] = b"d0:12:zero_len_key8:location17:udp://test.com:8011:nested dictd4:listli-500500eee6:numberi500500ee";
    const RECURSION: &'static [u8] = b"lllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllleeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const BYTES_UTF8: &'static [u8] = b"16:valid_utf8_bytes";
    const DICTIONARY: &'static [u8] = b"d9:test_dictd10:nested_key12:nested_value11:nested_listli500ei-500ei0eee8:test_key10:test_valuee";
    const LIST: &'static [u8] =
        b"l10:test_bytesi500ei0ei-500el12:nested_bytesed8:test_key10:test_valueee";
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
        assert_eq!(ben_dict.lookup("".as_bytes()).unwrap().str().unwrap(),
                   "zero_len_key");
        assert_eq!(ben_dict.lookup("location".as_bytes()).unwrap().str().unwrap(),
                   "udp://test.com:80");
        assert_eq!(ben_dict.lookup("number".as_bytes()).unwrap().int().unwrap(),
                   500500i64);

        let nested_dict = ben_dict.lookup("nested dict".as_bytes()).unwrap().dict().unwrap();
        let nested_list = nested_dict.lookup("list".as_bytes()).unwrap().list().unwrap();
        assert_eq!(nested_list[0].int().unwrap(), -500500i64);
    }

    #[test]
    fn positive_decode_recursion() {
        let _ = Bencode::decode(RECURSION).unwrap();

        // As long as we didnt overflow our call stack, we are good!
    }

    #[test]
    fn positive_decode_bytes_utf8() {
        let bencode = Bencode::decode(BYTES_UTF8).unwrap();

        assert_eq!(bencode.str().unwrap(), "valid_utf8_bytes");
    }

    #[test]
    fn positive_decode_dict() {
        let bencode = Bencode::decode(DICTIONARY).unwrap();
        let dict = bencode.dict().unwrap();
        assert_eq!(dict.lookup("test_key".as_bytes()).unwrap().str().unwrap(),
                   "test_value");

        let nested_dict = dict.lookup("test_dict".as_bytes()).unwrap().dict().unwrap();
        assert_eq!(nested_dict.lookup("nested_key".as_bytes()).unwrap().str().unwrap(),
                   "nested_value");

        let nested_list = nested_dict.lookup("nested_list".as_bytes()).unwrap().list().unwrap();
        assert_eq!(nested_list[0].int().unwrap(), 500i64);
        assert_eq!(nested_list[1].int().unwrap(), -500i64);
        assert_eq!(nested_list[2].int().unwrap(), 0i64);
    }

    #[test]
    fn positive_decode_list() {
        let bencode = Bencode::decode(LIST).unwrap();
        let list = bencode.list().unwrap();

        assert_eq!(list[0].str().unwrap(), "test_bytes");
        assert_eq!(list[1].int().unwrap(), 500i64);
        assert_eq!(list[2].int().unwrap(), 0i64);
        assert_eq!(list[3].int().unwrap(), -500i64);

        let nested_list = list[4].list().unwrap();
        assert_eq!(nested_list[0].str().unwrap(), "nested_bytes");

        let nested_dict = list[5].dict().unwrap();
        assert_eq!(nested_dict.lookup("test_key".as_bytes()).unwrap().str().unwrap(),
                   "test_value");
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
        let int_value = super::decode_int(INT, 1, ::BEN_END).unwrap().0;
        assert_eq!(int_value, 500i64);
    }

    #[test]
    fn positive_decode_int_negative() {
        let int_value = super::decode_int(INT_NEGATIVE, 1, ::BEN_END).unwrap().0;
        assert_eq!(int_value, -500i64);
    }

    #[test]
    fn positive_decode_int_zero() {
        let int_value = super::decode_int(INT_ZERO, 1, ::BEN_END).unwrap().0;
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
        super::decode_int(INT_NAN, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_int_leading_zero() {
        super::decode_int(INT_LEADING_ZERO, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_int_double_zero() {
        super::decode_int(INT_DOUBLE_ZERO, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_int_negative_zero() {
        super::decode_int(INT_NEGATIVE_ZERO, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_int_double_negative() {
        super::decode_int(INT_DOUBLE_NEGATIVE, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_dict_unordered_keys() {
        Bencode::decode(DICT_UNORDERED_KEYS).unwrap();
    }

    #[test]
    #[should_panic]
    fn negative_decode_dict_dup_keys_same_data() {
        Bencode::decode(DICT_DUP_KEYS_SAME_DATA).unwrap();
    }

    #[test]
    #[should_panic]
    fn negative_decode_dict_dup_keys_diff_data() {
        Bencode::decode(DICT_DUP_KEYS_DIFF_DATA).unwrap();
    }
}
