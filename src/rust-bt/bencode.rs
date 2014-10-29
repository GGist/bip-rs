use std::{i64};
use std::str::{Slice, MaybeOwned};
use std::io::{BufReader, SeekCur};
use std::collections::{HashMap};
use std::path::{BytesContainer};

pub type BenResult<T> = Result<T, MaybeOwned<'static>>;

const BEN_END: char = 'e';
const DICT_START: char = 'd';
const LIST_START: char = 'l';
const INT_START: char = 'i';
    
const BYTE_LEN_LOW: char = '1';
const BYTE_LEN_HIGH: char = '9';
const BYTE_LEN_END: char = ':';
    
pub enum BenValue {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<BenValue>),
    Dict(HashMap<String, BenValue>)
}

impl BenValue {
    pub fn new(bytes: &[u8]) -> BenResult<BenValue> {
        let buf = &mut BufReader::new(bytes);
        
        decode(buf)
    }
        
    pub fn encoded(&self) -> Vec<u8> {
        encode(self)
    }

    pub fn int<'a>(&'a self) -> Option<&'a i64> {
        match self {
            &Int(ref n) => Some(n),
            _           => None
        }
    }
    
    pub fn bytes<'a>(&'a self) -> Option<&'a [u8]> {
        match self {
            &Bytes(ref n) => Some(n.slice_from(0)),
            _             => None
        }
    }
    
    pub fn str<'a>(&'a self) -> Option<&'a str> {
        match self {
            &Bytes(ref n) => n.container_as_str(),
            _             => None
        }
    }

    pub fn list<'a>(&'a self) -> Option<&'a Vec<BenValue>> {
        match self {
            &List(ref n) => Some(n),
            _            => None
        }
    }

    pub fn dict<'a>(&'a self) -> Option<&'a HashMap<String, BenValue>> {
        match self {
            &Dict(ref n) => Some(n),
            _            => None
        }
    }
}
    
fn encode(val: &BenValue) -> Vec<u8> {
    match val {
        &Int(ref n) => encode_int(n),
        &Bytes(ref n) => encode_bytes(n.as_slice()),
        &List(ref n) => encode_list(n),
        &Dict(ref n) => encode_dict(n),
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
    
fn encode_list(list: &Vec<BenValue>) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    
    bytes.push(LIST_START as u8);
    for i in list.iter() {
        bytes.push_all(encode(i).as_slice());
    }
    bytes.push(BEN_END as u8);
    
    bytes
}
    
fn encode_dict(dict: &HashMap<String, BenValue>) -> Vec<u8> {
    // Need To Sort The Keys In The Map Before Encoding
    let mut sort_dict: Vec<(&String, &BenValue)> = Vec::new();
    let mut bytes: Vec<u8> = Vec::new();
    
    // Keep Iterator Alive For Current Scope
    let mut map_iter = dict.iter();
    // Store References That Are Pointing Into The Iterator
    for (key, value) in map_iter {
        sort_dict.push((key, value));
    }
        
    sort_dict.sort_by(|&(a, _), &(b, _)| a.cmp(b));
        
    bytes.push(DICT_START as u8);
    // Iterate And Dictionary Encode The String/BenValue Pairs
    for &(ref key, ref value) in sort_dict.iter() {
        bytes.push_all(encode_bytes(key.as_bytes()).as_slice());
        bytes.push_all(encode(*value).as_slice());
    }
    bytes.push(BEN_END as u8);
    
    bytes
}
    
fn decode(buf: &mut BufReader) -> BenResult<BenValue> {
    let curr_char = try!(buf.read_char().or_else({ |e|
        Err(Slice(e.desc))
    }));
    
    let ben_val = match curr_char {
        INT_START  => Int(try!(decode_int(buf, BEN_END))),
        LIST_START => List(try!(decode_list(buf))),
        DICT_START => Dict(try!(decode_dict(buf))),
        BYTE_LEN_LOW...BYTE_LEN_HIGH => {
            // Back Stream Up So That First Digit Is Included
            try!(buf.seek(-1, SeekCur).or_else({ |e|
                Err(Slice(e.desc))
            }))
            Bytes(try!(decode_bytes(buf)))
        },
        _ => return Err(Slice("Unknown BenValue Identifier Encountered"))
    };
    
    Ok(ben_val)
}
    
fn decode_int(buf: &mut BufReader, delim: char) -> BenResult<i64> {
    let delim = delim as u8;
    let mut int_bytes = try!(buf.read_until(delim).or_else({ |e|
        Err(Slice(e.desc))
    }));
    
    match int_bytes.pop() {
        Some(_) => (),
        None    => return Err(Slice("Empty String Parse Encountered"))
    };
    
    match i64::parse_bytes(int_bytes.as_slice(), 10) {
        Some(n) => Ok(n),
        None    => return Err(Slice("Could Not Parse i64 From Bytes"))
    }
}
    
fn decode_bytes(buf: &mut BufReader) -> BenResult<Vec<u8>> {
    let num_bytes = try!(decode_int(buf, BYTE_LEN_END)) as uint;
    
    match buf.read_exact(num_bytes) {
        Ok(n)  => Ok(n),
        Err(n) => Err(Slice(n.desc))
    }
}

fn decode_list(buf: &mut BufReader) -> BenResult<Vec<BenValue>> {
    let mut ben_list: Vec<BenValue> = Vec::new();
    
    while try!(peek_char(buf)) != BEN_END {
        ben_list.push(try!(decode(buf)));
    }
    buf.consume(1);
    
    Ok(ben_list)
}
    
fn decode_dict(buf: &mut BufReader) -> BenResult<HashMap<String, BenValue>> {
    let mut ben_dict: HashMap<String, BenValue> = HashMap::new();
    
    while try!(peek_char(buf)) != BEN_END {
        let key = match try!(decode_bytes(buf)).into_ascii_opt() {
            Some(n) => n.into_string(),
            None    => return Err(Slice("Key Is Not A Valid UTF-8 String"))
        };
        
        let val = try!(decode(buf));
        
        ben_dict.insert(key, val);
    }
    buf.consume(1);
    
    Ok(ben_dict)
}
    
fn peek_char(buf: &mut BufReader) -> BenResult<char> {
    let next_char = try!(buf.read_char().or_else({ |e|
        Err(Slice(e.desc))
    }));
    
    match buf.seek(-1, SeekCur) {
        Ok(_)  => Ok(next_char),
        Err(_) => Err(Slice("Could Not Seek Backwards By 1 In Buffer"))
    }
}