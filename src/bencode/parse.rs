use std::collections::{HashMap};
use std::collections::hash_map::{Entry};
use std::iter::{Peekable};

use bencode::{Bencode, Bencoded, BencodedKind};
use error::{BencodeResult, BencodeError, BencodeErrorKind};
use util::{Dictionary};

const BEN_END:    u8 = b'e';
const DICT_START: u8 = b'd';
const LIST_START: u8 = b'l';
const INT_START:  u8 = b'i';

const BYTE_LEN_LOW:  u8 = b'0';
const BYTE_LEN_HIGH: u8 = b'9';
const BYTE_LEN_END:  u8 = b':';

pub fn encode<T>(val: T<Output=T>) -> Vec<u8> 
    where T: Bencoded {
    match val.kind() {
        BencodedKind::Int(n)   => encode_int(n),
        BencodedKind::Bytes(n) => encode_bytes(&n),
        BencodedKind::List(n)  => encode_list(n),
        BencodedKind::Dict(n)  => encode_dict(n)
    }
}
    
fn encode_int(val: i64) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    
    bytes.push(INT_START);
    bytes.push_all(val.to_string().as_bytes());
    bytes.push(BEN_END);
    
    bytes
}
    
fn encode_bytes(list: &[u8]) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    
    bytes.push_all(list.len().to_string().as_bytes());
    bytes.push(BYTE_LEN_END);
    bytes.push_all(list);
    
    bytes
}
    
fn encode_list<T>(list: &[T]) -> Vec<u8>
    where T: Bencoded {
    let mut bytes: Vec<u8> = Vec::new();
    
    bytes.push(LIST_START);
    for i in list {
        bytes.push_all(&encode(i));
    }
    bytes.push(BEN_END);
    
    bytes
}
    
fn encode_dict<T>(dict: &Dictionary<String, T>) -> Vec<u8>
    where T: Bencoded {
    // Need To Sort The Keys In The Map Before Encoding
    let mut bytes: Vec<u8> = Vec::new();

    let mut sort_dict = dict.to_list();
    sort_dict.sort_by(|&(a, _), &(b, _)| a.cmp(b));
        
    bytes.push(DICT_START);
    // Iterate And Dictionary Encode The (String, Bencode) Pairs
    for &(ref key, ref value) in sort_dict.iter() {
        bytes.push_all(&encode_bytes(key.as_bytes()));
        bytes.push_all(&encode(*value));
    }
    bytes.push(BEN_END);
    
    bytes
}


pub fn decode<T>(bytes: &mut Peekable<T>) -> BencodeResult<Bencode>
    where T: Iterator<Item=(usize, u8)> {
    let &(curr_pos, curr_char) = try!(bytes.peek().ok_or(
        BencodeError::new(BencodeErrorKind::BytesEmpty, "Stopped At Start Of Decode", None)
    ));
    
    match curr_char {
        INT_START  => {
            bytes.next();
            Ok(Bencode::Int(try!(decode_int(bytes, BEN_END))))
        },
        LIST_START => {
            bytes.next();
            Ok(Bencode::List(try!(decode_list(bytes))))
        },
        DICT_START => {
            bytes.next();
            Ok(Bencode::Dict(try!(decode_dict(bytes))))
        },
        BYTE_LEN_LOW...BYTE_LEN_HIGH => {
            // Include The Length Digit, Don't Consume It
            Ok(Bencode::Bytes(try!(decode_bytes(bytes))))
        },
        _ => Err(BencodeError::new(BencodeErrorKind::InvalidByte, 
                                   "Unknown Bencode Type Token Found",
                                   Some(curr_pos)))
    }
}
    
fn decode_int<T>(bytes: &mut Peekable<T>, delim: u8) -> BencodeResult<i64>
    where T: Iterator<Item=(usize, u8)> {
    let curr_pos = try!(peek_position(bytes, "Stopped At Start Of Integer Decode"));
    let int_bytes: Vec<u8> = bytes.map(|(_, byte)| byte)
                                  .take_while(|&byte| byte != delim)
                                  .collect();
    
    // Explicit error checking
    if int_bytes.len() > 1 {
        // Zero padding is illegal for integers, and unspecified for lengths (disallow both)
        if int_bytes[0] == b'0' {
            return Err(BencodeError::new(BencodeErrorKind::InvalidInt,
                            "Illegal Zero Padding On Integer/Length", Some(curr_pos)))
        }
    
        // Negative zero is not allowed (but would pass as valid if parsed as an integer)
        if int_bytes[0] == b'-' && int_bytes[1] == b'0' {
            return Err(BencodeError::new(BencodeErrorKind::InvalidInt,
                            "Illegal Negative Zero", Some(curr_pos)))
        }
    }
    
    let int_str = String::from_utf8_lossy(&int_bytes[..]);
    match i64::from_str_radix(&*int_str, 10) {
        Ok(n) => Ok(n),
        Err(_) => Err(BencodeError::new(BencodeErrorKind::InvalidInt,
            "Could Not Convert Integer To i64", Some(curr_pos)))
    }
}
    
fn decode_bytes<T>(bytes: &mut Peekable<T>) -> BencodeResult<Vec<u8>>
    where T: Iterator<Item=(usize, u8)> {
    let curr_pos = try!(peek_position(bytes, "Stopped At Start Of Bytes Decode"));
    let num_bytes = try!(decode_int(bytes, BYTE_LEN_END));

    if num_bytes < 0 {
        return Err(BencodeError::new(BencodeErrorKind::InvalidLength, 
                                     "Negative Length Bytes Found", Some(curr_pos)))
    }
    
    let owned_bytes = bytes.take(num_bytes as usize)
                           .map(|(_, byte)| byte)
                           .collect::<Vec<u8>>();

    if owned_bytes.len() == num_bytes as usize {
        Ok(owned_bytes)
    } else {
        Err(BencodeError::new(BencodeErrorKind::BytesEmpty,
                              "Byte Length Ran Past EOF", Some(curr_pos)))
    }
}

fn decode_list<T>(bytes: &mut Peekable<T>) -> BencodeResult<Vec<Bencode>>
    where T: Iterator<Item=(usize, u8)> {
    let mut ben_list: Vec<Bencode> = Vec::new();
    
    let mut curr_byte = try!(peek_byte(bytes, "Stopped At List Element"));
    while curr_byte != BEN_END {
        ben_list.push(try!(decode(bytes)));
        
        curr_byte = try!(peek_byte(bytes, "Stopped At List Element"));
    }
    bytes.next();
    
    Ok(ben_list)
}
    
fn decode_dict<T>(bytes: &mut Peekable<T>) -> BencodeResult<HashMap<String, Bencode>>
    where T: Iterator<Item=(usize, u8)> {
    let mut ben_dict: HashMap<String, Bencode> = HashMap::new();
    let curr_pos = try!(peek_position(bytes, "Stopped At Dict Element"));
    
    let mut last_key = String::with_capacity(0);
    let mut curr_byte = try!(peek_byte(bytes, "Stopped At Dict Element"));
    
    while curr_byte != BEN_END {
        let key = match String::from_utf8(try!(decode_bytes(bytes))) {
            Ok(n) => n,
            Err(e) => {
                return Err(BencodeError::new(BencodeErrorKind::InvalidByte,
                    "Dictionary Key Is Not Valid UTF-8", Some(curr_pos)))
            }
        };
        
        // Spec says that the keys must be in alphabetical order
        if last_key.len() != 0 && key < last_key {
            return Err(BencodeError::new(BencodeErrorKind::InvalidKey,
                "Dictionary Key Not In Alphabetical Order", Some(curr_pos)))
        }
        
        let val = try!(decode(bytes));
        match ben_dict.entry(key.clone()) {
            Entry::Vacant(n)   => n.insert(val),
            Entry::Occupied(_) => {
                return Err(BencodeError::new(BencodeErrorKind::InvalidKey,
                    "Duplicate Key Found", Some(curr_pos)))
            }
        };

        last_key = key;
        curr_byte = try!(peek_byte(bytes, "Stopped At Dict Element"));
    }
    bytes.next();
    
    Ok(ben_dict)
}

fn peek_byte<T>(bytes: &mut Peekable<T>, err_msg: &'static str) -> BencodeResult<u8>
    where T: Iterator<Item=(usize, u8)> {
    bytes.peek().map(|&(_, n)| n).ok_or(
        BencodeError::new(BencodeErrorKind::BytesEmpty, err_msg, None)
    )
}

fn peek_position<T>(bytes: &mut Peekable<T>, err_msg: &'static str) -> BencodeResult<usize>
    where T: Iterator<Item=(usize, u8)> {
    bytes.peek().map(|&(n, _)| n).ok_or(
        BencodeError::new(BencodeErrorKind::BytesEmpty, err_msg, None)
    )
}

#[cfg(test)]
mod tests {
    use std::io::{Read};
    
    use bencode::{Bencoded};
    use super::{BEN_END};

    // Positive Cases
    const DICTIONARY: &'static [u8] = b"d9:test_dictd10:nested_key12:nested_value11:nested_listli500ei-500ei0eee8:test_key10:test_valuee";
    const LIST: &'static [u8] = b"l10:test_bytesi500ei0ei-500el12:nested_bytesed8:test_key10:test_valueee";
    const BYTES: &'static [u8] = b"5:\xC5\xE6\xBE\xE6\xF2";
    const BYTES_ZERO_LEN: &'static [u8] = b"0:";
    const INT: &'static [u8] = b"i500e";
    const INT_NEGATIVE: &'static [u8] = b"i-500e";
    const INT_ZERO: &'static [u8] = b"i0e";
    
    // Negative Cases
    const INT_NAN: &'static [u8] = b"i500a500e";
    const INT_LEADING_ZERO: &'static [u8] = b"i0500e";
    const INT_DOUBLE_ZERO: &'static [u8] = b"i00e";
    const INT_NEGATIVE_ZERO: &'static [u8] = b"i-0e";
    const INT_DOUBLE_NEGATIVE: &'static [u8] = b"i--5e";
    const DICT_UNORDERED_KEYS: &'static [u8] = b"d5:z_key5:value5:a_key5:valuee";
    const DICT_DUPLICATE_KEYS_SAME_DATA: &'static [u8] = b"d5:a_keyi0e5:a_keyi0ee";
    const DICT_DUPLICATE_KEYS_DIFF_DATA: &'static [u8] = b"d5:a_keyi0e5:a_key7:a_valuee";
    
    #[test]
    fn positive_decode_dict() {
        let mut buf = DICTIONARY.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        let dict = super::decode_dict(&mut buf).unwrap();
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
        let mut buf = LIST.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        let list = super::decode_list(&mut buf).unwrap();
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
        let mut buf = BYTES.iter().map(|&n| n).enumerate().peekable();
        
        let bytes = super::decode_bytes(&mut buf).unwrap();
        assert_eq!(bytes.len(), 5);
        assert_eq!(bytes[0] as char, 'Å');
        assert_eq!(bytes[1] as char, 'æ');
        assert_eq!(bytes[2] as char, '¾');
        assert_eq!(bytes[3] as char, 'æ');
        assert_eq!(bytes[4] as char, 'ò');
    }
    
    #[test]
    fn positive_decode_bytes_zero_len() {
        let mut buf = BYTES_ZERO_LEN.iter().map(|&n| n).enumerate().peekable();
        
        let bytes = super::decode_bytes(&mut buf).unwrap();
        assert_eq!(bytes.len(), 0);
    }
   
    #[test]
    fn positive_decode_int() {
        let mut buf = INT.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        let int_value = super::decode_int(&mut buf, BEN_END).unwrap();
        assert_eq!(int_value, 500i64);
    }
   
    #[test]
    fn positive_decode_int_negative() {
        let mut buf = INT_NEGATIVE.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        let int_value = super::decode_int(&mut buf, BEN_END).unwrap();
        assert_eq!(int_value, -500i64);
    }
    
    #[test]
    fn positive_decode_int_zero() {
        let mut buf = INT_ZERO.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        let int_value = super::decode_int(&mut buf, BEN_END).unwrap();
        assert_eq!(int_value, 0i64);
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_nan() {
        let mut buf = INT_NAN.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        super::decode_int(&mut buf, BEN_END).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_leading_zero() {
        let mut buf = INT_LEADING_ZERO.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        super::decode_int(&mut buf, BEN_END).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_double_zero() {
        let mut buf = INT_DOUBLE_ZERO.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        super::decode_int(&mut buf, BEN_END).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_negative_zero() {
        let mut buf = INT_NEGATIVE_ZERO.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        super::decode_int(&mut buf, BEN_END).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_int_double_negative() {
        let mut buf = INT_DOUBLE_NEGATIVE.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        super::decode_int(&mut buf, BEN_END).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_dict_unordered_keys() {
        let mut buf = DICT_UNORDERED_KEYS.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        super::decode_dict(&mut buf).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_dict_duplicate_keys_same_data() {
        let mut buf = DICT_DUPLICATE_KEYS_SAME_DATA.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        super::decode_dict(&mut buf).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_dict_duplicate_keys_diff_data() {
        let mut buf = DICT_DUPLICATE_KEYS_DIFF_DATA.iter().map(|&n| n).enumerate().peekable();
        buf.next().unwrap();
        
        super::decode_dict(&mut buf).unwrap();
    }
}