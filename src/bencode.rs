use std::io::{BufReader, SeekCur};
use std::str::Utf8Error::{InvalidByte, TooShort};
use std::collections::{HashMap};
use std::path::{BytesContainer};
use error::{ParseResult, ParseError};

const BEN_END: char = 'e';
const DICT_START: char = 'd';
const LIST_START: char = 'l';
const INT_START: char = 'i';
    
const BYTE_LEN_LOW: char = '0';
const BYTE_LEN_HIGH: char = '9';
const BYTE_LEN_END: char = ':';

/// Structure representing bencoded data.
#[derive(Show)]
pub enum Bencode {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<Bencode>),
    Dict(HashMap<String, Bencode>)
}

impl Bencode {
   /// Processes bytes as bencoded data and builds a Bencode structure to represent
   /// the bencoded bytes.
    ///
    /// All valid bencode will be accepted. However, any valid bencode containing 
    /// extra, unprocessed bytes at the end will be considered invalid.
    pub fn new(bytes: &[u8]) -> ParseResult<Bencode> {
        let buf = &mut BufReader::new(bytes);
        let result = decode(buf);
        
        if !buf.eof() {
            return Err(ParseError::new(try!(buf.tell()), "End Portion Of bytes Not Processed", None))
        }
        
        result
    }
    
   /// Serializes the Bencode data structure back into a sequence of bytes.
    ///
    /// The returned byte sequence is guaranteed to be the same byte sequence 
    /// that was initially used to build the Bencode object.
    pub fn encoded(&self) -> Vec<u8> {
        encode(self)
    }

   /// Tries to convert the current Bencode value to an i64.
    pub fn int(&self) -> Option<i64> {
        match self {
            &Bencode::Int(n) => Some(n),
            _                => None
        }
    }
    
   /// Tries to convert the current Bencode value to a sequence of bytes.
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            &Bencode::Bytes(ref n) => Some(n.slice_from(0)),
            _                      => None
        }
    }
    
   /// Tries to convert the current Bencode value to a str (only valid UTF-8
    /// byte sequences are convertible).
    pub fn str(&self) -> Option<&str> {
        match self {
            &Bencode::Bytes(ref n) => n.container_as_str(),
            _                      => None
        }
    }

   /// Tries to convert the current Bencode value to a list of Bencoded values.
    pub fn list(&self) -> Option<&Vec<Bencode>> {
        match self {
            &Bencode::List(ref n) => Some(n),
            _                     => None
        }
    }

   /// Tries to convert the current Bencode value to a dictionary of Bencoded 
    /// values.
    pub fn dict(&self) -> Option<&HashMap<String, Bencode>> {
        match self {
            &Bencode::Dict(ref n) => Some(n),
            _                     => None
        }
    }
}
    
fn encode(val: &Bencode) -> Vec<u8> {
    match val {
        &Bencode::Int(ref n)   => encode_int(n),
        &Bencode::Bytes(ref n) => encode_bytes(n.as_slice()),
        &Bencode::List(ref n)  => encode_list(n),
        &Bencode::Dict(ref n)  => encode_dict(n)
    }
}
    
fn encode_int(val: &i64) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    
    bytes.push(INT_START as u8);
    bytes.push_all(val.to_string().container_as_bytes());
    bytes.push(BEN_END as u8);
    
    bytes
}
    
fn encode_bytes(list: &[u8]) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    
    bytes.push_all(list.len().to_string().container_as_bytes());
    bytes.push(':' as u8);
    bytes.push_all(list);
    
    bytes
}
    
fn encode_list(list: &Vec<Bencode>) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    
    bytes.push(LIST_START as u8);
    for i in list.iter() {
        bytes.push_all(encode(i).as_slice());
    }
    bytes.push(BEN_END as u8);
    
    bytes
}
    
fn encode_dict(dict: &HashMap<String, Bencode>) -> Vec<u8> {
    // Need To Sort The Keys In The Map Before Encoding
    let mut sort_dict: Vec<(&String, &Bencode)> = Vec::new();
    let mut bytes: Vec<u8> = Vec::new();
    
    let mut map_iter = dict.iter();
    // Store References That Are Pointing Into The Iterator
    for (key, value) in map_iter {
        sort_dict.push((key, value));
    }
        
    sort_dict.sort_by(|&(a, _), &(b, _)| a.cmp(b));
        
    bytes.push(DICT_START as u8);
    // Iterate And Dictionary Encode The (String, Bencode) Pairs
    for &(ref key, ref value) in sort_dict.iter() {
        bytes.push_all(encode_bytes(key.as_bytes()).as_slice());
        bytes.push_all(encode(*value).as_slice());
    }
    bytes.push(BEN_END as u8);
    
    bytes
}
    
fn decode(buf: &mut BufReader) -> ParseResult<Bencode> {
    let curr_char = try!(buf.read_char().or_else({ |e|
        Err(ParseError::new(try!(buf.tell()), e.desc, e.detail))
    }));
    
    let ben_val = match curr_char {
        INT_START  => Bencode::Int(try!(decode_int(buf, BEN_END))),
        LIST_START => Bencode::List(try!(decode_list(buf))),
        DICT_START => Bencode::Dict(try!(decode_dict(buf))),
        BYTE_LEN_LOW...BYTE_LEN_HIGH => {
            // Back Stream Up So That First Digit Is Included
            try!(buf.seek(-1, SeekCur).or_else({ |e|
                Err(ParseError::new(try!(buf.tell()), e.desc, e.detail))
            }));
            Bencode::Bytes(try!(decode_bytes(buf)))
        },
        _ => return Err(ParseError::new(try!(buf.tell()), "Unknown Bencode Type Token Found", None))
    };
    
    Ok(ben_val)
}
    
fn decode_int(buf: &mut BufReader, delim: char) -> ParseResult<i64> {
    let delim = delim as u8;
    let mut int_bytes = try!(buf.read_until(delim).or_else({ |e|
        Err(ParseError::new(try!(buf.tell()), e.desc, e.detail))
    }));
    
    match int_bytes.pop() {
        Some(_) => (),
        None    => return Err(ParseError::new(try!(buf.tell()), "Empty Integer Delimiter Encountered", None))
    };
    
    // Zero padding is illegal for integers, and unspecified for lengths (disallow both)
    if int_bytes.len() > 1 && int_bytes[0] == b'0' {
        return Err(ParseError::new(try!(buf.tell()), "Illegal Zero Padding On Integer/Length", None))
    }
    
    let int_str = try!(int_bytes.container_as_str().ok_or(
        ParseError::new(try!(buf.tell()), "Could Not Parse Integer As UTF-8", None)
    ));
    match int_str.parse::<i64>() {
        Some(n) => Ok(n),
        None    => return Err(ParseError::new(try!(buf.tell()), "Could Not Convert Integer To i64", None))
    }
}
    
fn decode_bytes(buf: &mut BufReader) -> ParseResult<Vec<u8>> {
    let num_bytes = try!(decode_int(buf, BYTE_LEN_END));

    if num_bytes < 0 {
        return Err(ParseError::new(try!(buf.tell()), "Negative Length String Found", None))
    }
    
    match buf.read_exact(num_bytes as uint) {
        Ok(n)  => Ok(n),
        Err(e) => Err(ParseError::new(try!(buf.tell()), e.desc, e.detail))
    }
}

fn decode_list(buf: &mut BufReader) -> ParseResult<Vec<Bencode>> {
    let mut ben_list: Vec<Bencode> = Vec::new();
    
    while try!(peek_char(buf)) != BEN_END {
        ben_list.push(try!(decode(buf)));
    }
    buf.consume(1);
    
    Ok(ben_list)
}
    
fn decode_dict(buf: &mut BufReader) -> ParseResult<HashMap<String, Bencode>> {
    let mut ben_dict: HashMap<String, Bencode> = HashMap::new();
    
    let mut last_key = String::with_capacity(0);
    while try!(peek_char(buf)) != BEN_END {
        let key = match String::from_utf8(try!(decode_bytes(buf))) {
            Ok(n) => n,
            Err(e) => {
                let position: u64 = match e.utf8_error() {
                    InvalidByte(s) => try!(buf.tell()) - e.into_bytes().len() as u64 + s as u64,
                    TooShort       => try!(buf.tell()) - e.into_bytes().len() as u64
                };
                return Err(ParseError::new(position, "Dictionary Key Is Not Valid UTF-8", None))
            }
        };
        
        // Spec says that the keys must be in alphabetical order
        if last_key.len() != 0 && key < last_key {
            return Err(ParseError::new(try!(buf.tell()), "Dictionary Key Not In Alphabetical Order", None))
        }
        
        let val = try!(decode(buf));
        ben_dict.insert(key.clone(), val);

        last_key = key;
    }
    buf.consume(1);
    
    Ok(ben_dict)
}
    
fn peek_char(buf: &mut BufReader) -> ParseResult<char> {
    let next_char = try!(buf.read_char().or_else({ |e|
        Err(ParseError::new(try!(buf.tell()), e.desc, e.detail))
    }));
    
    match buf.seek(-1, SeekCur) {
        Ok(_)  => Ok(next_char),
        Err(_) => Err(ParseError::new(try!(buf.tell()), "Could Not Move Buffer Cursor Back One", None))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, SeekCur};
    use super::{Bencode, decode_dict, decode_list, decode_bytes, decode_int, BEN_END};

    // Positive Cases
    const GENERAL: &'static [u8] = b"d0:12:zero_len_key8:location17:udp://test.com:8011:nested dictd4:listli-500500eee6:numberi500500ee";
    const DICTIONARY: &'static [u8] = b"d9:test_dictd10:nested_key12:nested_value11:nested_listli500ei-500ei0eee8:test_key10:test_valuee";
    const LIST: &'static [u8] = b"l10:test_bytesi500ei0ei-500el12:nested_bytesed8:test_key10:test_valueee";
    const BYTES: &'static [u8] = b"5:\xC5\xE6\xBE\xE6\xF2";
    const BYTES_UTF8: &'static [u8] = b"16:valid_utf8_bytes";
    const BYTES_ZERO_LEN: &'static [u8] = b"0:";
    const INT: &'static [u8] = b"i500e";
    const INT_NEGATIVE: &'static [u8] = b"i-500e";
    const INT_ZERO: &'static [u8] = b"i0e";
   
    // Negative Cases
    const BYTES_NEG_LEN: &'static [u8] = b"-4:test";
    const BYTES_EXTRA: &'static [u8] = b"l15:processed_bytese17:unprocessed_bytes";
    const INT_INVALID: &'static [u8] = b"i500a500e";
    const INT_LEADING_ZERO: &'static [u8] = b"i0500e";
    const INT_DOUBLE_ZERO: &'static [u8] = b"i00e";
    const BYTES_NOT_UTF8: &'static [u8] = b"5:\xC5\xE6\xBE\xE6\xF2";
    const DICT_UNORDERED_KEYS: &'static [u8] = b"d5:z_key5:value5:a_key5:valuee";
   
   #[test]
   fn positive_decode_general() {
        let bencode = Bencode::new(GENERAL).unwrap();
        
        let ben_dict = bencode.dict().unwrap();
        assert_eq!(ben_dict.get("").unwrap().str().unwrap(), "zero_len_key");
        assert_eq!(ben_dict.get("location").unwrap().str().unwrap(), "udp://test.com:80");
        assert_eq!(ben_dict.get("number").unwrap().int().unwrap(), 500500i64);
        
        let nested_dict = ben_dict.get("nested dict").unwrap().dict().unwrap();
        let nested_list = nested_dict.get("list").unwrap().list().unwrap();
        assert_eq!(nested_list[0].int().unwrap(), -500500i64);
   }
   
   #[test]
   fn positive_decode_dict() {
        let mut buf = BufReader::new(DICTIONARY);
        buf.seek(1, SeekCur).unwrap();
        
        let dict = decode_dict(&mut buf).unwrap();
        assert_eq!(dict.get("test_key").unwrap().str().unwrap(), "test_value");
        
        let nested_dict = dict.get("test_dict").unwrap().dict().unwrap();
        assert_eq!(nested_dict.get("nested_key").unwrap().str().unwrap(), "nested_value");
        
        let nested_list = nested_dict.get("nested_list").unwrap().list().unwrap();
        assert_eq!(nested_list[0].int().unwrap(), 500i64);
        assert_eq!(nested_list[1].int().unwrap(), -500i64);
        assert_eq!(nested_list[2].int().unwrap(), 0i64);
   }
   
   #[test]
   fn positive_decode_list() {
        let mut buf = BufReader::new(LIST);
        buf.seek(1, SeekCur).unwrap();
        
        let list = decode_list(&mut buf).unwrap();
        assert_eq!(list[0].str().unwrap(), "test_bytes");
        assert_eq!(list[1].int().unwrap(), 500i64);
        assert_eq!(list[2].int().unwrap(), 0i64);
        assert_eq!(list[3].int().unwrap(), -500i64);
        
        let nested_list = list[4].list().unwrap();
        assert_eq!(nested_list[0].str().unwrap(), "nested_bytes");
        
        let nested_dict = list[5].dict().unwrap();
        assert_eq!(nested_dict.get("test_key").unwrap().str().unwrap(), "test_value");
   }
   
   #[test]
   fn positive_decode_bytes() {
        let mut buf = BufReader::new(BYTES);
        
        let bytes = decode_bytes(&mut buf).unwrap();
        assert_eq!(bytes.len(), 5);
        assert_eq!(bytes[0] as char, 'Å');
        assert_eq!(bytes[1] as char, 'æ');
        assert_eq!(bytes[2] as char, '¾');
        assert_eq!(bytes[3] as char, 'æ');
        assert_eq!(bytes[4] as char, 'ò');
   }
    
   #[test]
   fn positive_decode_bytes_utf8() {
        let bencode = Bencode::new(BYTES_UTF8).unwrap();
        
        assert_eq!(bencode.str().unwrap(), "valid_utf8_bytes");
   }
   
   #[test]
   fn positive_decode_bytes_zero_len() {
        let mut buf = BufReader::new(BYTES_ZERO_LEN);
        
        let bytes = decode_bytes(&mut buf).unwrap();
        assert_eq!(bytes.len(), 0);
   }
   
   #[test]
   fn positive_decode_int() {
        let mut buf = BufReader::new(INT);
        buf.seek(1, SeekCur).unwrap();
        
        let int_value = decode_int(&mut buf, BEN_END).unwrap();
        assert_eq!(int_value, 500i64);
   }
   
   #[test]
   fn positive_decode_int_negative() {
        let mut buf = BufReader::new(INT_NEGATIVE);
        buf.seek(1, SeekCur).unwrap();
        
        let int_value = decode_int(&mut buf, BEN_END).unwrap();
        assert_eq!(int_value, -500i64);
   }
    
    #[test]
    fn positive_decode_int_zero() {
        let mut buf = BufReader::new(INT_ZERO);
        buf.seek(1, SeekCur).unwrap();
        
        let int_value = decode_int(&mut buf, BEN_END).unwrap();
        assert_eq!(int_value, 0i64);
    }
    
    #[test]
    #[should_fail]
    fn negative_decode_bytes_neg_len() {
        Bencode::new(BYTES_NEG_LEN).unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_decode_bytes_extra() {
        Bencode::new(BYTES_EXTRA).unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_decode_int_invalid() {
        let mut buf = BufReader::new(INT_INVALID);
        buf.seek(1, SeekCur).unwrap();
        
        decode_int(&mut buf, BEN_END).unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_decode_int_leading_zero() {
        let mut buf = BufReader::new(INT_LEADING_ZERO);
        buf.seek(1, SeekCur).unwrap();
        
        decode_int(&mut buf, BEN_END).unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_decode_int_double_zero() {
        let mut buf = BufReader::new(INT_DOUBLE_ZERO);
        buf.seek(1, SeekCur).unwrap();
        
        decode_int(&mut buf, BEN_END).unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_decode_bytes_not_utf8() {
        let bencode = Bencode::new(BYTES_NOT_UTF8).unwrap();
        
        bencode.str().unwrap();
    }
    
    #[test]
    #[should_fail]
    fn negative_decode_dict_unordered_keys() {
        let mut buf = BufReader::new(DICT_UNORDERED_KEYS);
        buf.seek(1, SeekCur).unwrap();
        
        decode_dict(&mut buf).unwrap();
    }
}