use std::collections::{BTreeMap};
use std::collections::btree_map::{Entry};
use std::str::{self};

use bencode::{Bencode};
use error::{BencodeParseError, BencodeParseErrorKind, BencodeParseResult};

// Storage for regular bencode as well as mappings (the invariant is that the
// underlying recursive type is a dictionary, it is a programming error otherwise)
enum IBencodeType<'a> {
    Bencode(Bencode<'a>),
    BencodeMapping(&'a [u8], Bencode<'a>)
}

/// Decodes the given list of bytes at the given position into a bencoded structures.
///
/// Panic only occurs is a programming error occurred.
pub fn decode<'a>(bytes: &'a [u8], mut pos: usize) -> BencodeParseResult<(Bencode<'a>, usize)> {
    let mut bencode_stack = Vec::new();
    let (shallow_type, new_pos) = try!(decode_shallow(bytes, pos));
    pos = new_pos;
    
    bencode_stack.push(IBencodeType::Bencode(shallow_type));
    
    // While the stack is not empty, keep pulling, parsing, collapsing, etc.
    while let Some(curr_ibencode) = bencode_stack.pop() {
        match curr_ibencode {
            ib_int @ IBencodeType::Bencode(Bencode::Int(_)) => {
                let opt_last_type = try!(collapse_bencode(&mut bencode_stack, ib_int, pos));
                
                if let Some(last_type) = opt_last_type {
                    return Ok((last_type, pos))
                }
            },
            ib_bytes @ IBencodeType::Bencode(Bencode::Bytes(_)) => {
                let opt_last_type = try!(collapse_bencode(&mut bencode_stack, ib_bytes, pos));
                
                if let Some(last_type) = opt_last_type {
                    return Ok((last_type, pos))
                }
            },
            ib_list @ IBencodeType::Bencode(Bencode::List(_)) => {
                let curr_byte = try!(peek_byte(bytes, pos, "End Of Bytes Encountered"));
                
                if curr_byte == ::BEN_END {
                    // Recursive type has reached it's end, collapse it now
                    pos += 1;
                    let opt_last_type = try!(collapse_bencode(&mut bencode_stack, ib_list, pos));
                    
                    if let Some(last_type) = opt_last_type {
                        return Ok((last_type, pos))
                    }
                } else {
                    // Recursive type still has entries, push it back and then push the entry onto the stack
                    bencode_stack.push(ib_list);
                    
                    let (shallow_type, new_pos) = try!(decode_shallow(bytes, pos));
                    pos = new_pos;
                    
                    bencode_stack.push(IBencodeType::Bencode(shallow_type));
                }
            },
            ib_dict @ IBencodeType::Bencode(Bencode::Dict(_)) => {
                let curr_byte = try!(peek_byte(bytes, pos, "End Of Bytes Encountered"));
                
                if curr_byte == ::BEN_END {
                    // Recursive type has reached it's end, collapse it now
                    pos += 1;
                    let opt_last_type = try!(collapse_bencode(&mut bencode_stack, ib_dict, pos));
                    
                    if let Some(last_type) = opt_last_type {
                        return Ok((last_type, pos))
                    }
                } else {
                    // Recursive type still has entries, push it back and then push the entry onto the stack
                    bencode_stack.push(ib_dict);
                    
                    let (key, new_pos) = try!(decode_key(bytes, pos));
                    pos = new_pos;
                    
                    let (shallow_type, new_pos) = try!(decode_shallow(bytes, pos));
                    pos = new_pos;
                    
                    bencode_stack.push(IBencodeType::BencodeMapping(key, shallow_type));
                }
            },
            ib_map_int @ IBencodeType::BencodeMapping(_, Bencode::Int(_)) => {
                let opt_last_type = try!(collapse_bencode(&mut bencode_stack, ib_map_int, pos));
                
                if let Some(last_type) = opt_last_type {
                    return Ok((last_type, pos))
                }
            },
            ib_map_bytes @ IBencodeType::BencodeMapping(_, Bencode::Bytes(_)) => {
                let opt_last_type = try!(collapse_bencode(&mut bencode_stack, ib_map_bytes, pos));
                
                if let Some(last_type) = opt_last_type {
                    return Ok((last_type, pos))
                }
            },
            ib_map_list @ IBencodeType::BencodeMapping(_, Bencode::List(_)) => {
                let curr_byte = try!(peek_byte(bytes, pos, "End Of Bytes Encountered"));
                
                if curr_byte == ::BEN_END {
                    // Recursive type has reached it's end, collapse it now
                    pos += 1;
                    let opt_last_type = try!(collapse_bencode(&mut bencode_stack, ib_map_list, pos));
                    
                    if let Some(last_type) = opt_last_type {
                        return Ok((last_type, pos))
                    }
                } else {
                    // Recursive type still has entries, push it back and then push the entry onto the stack
                    bencode_stack.push(ib_map_list);
                    
                    let (shallow_type, new_pos) = try!(decode_shallow(bytes, pos));
                    pos = new_pos;
                    
                    bencode_stack.push(IBencodeType::Bencode(shallow_type));
                }
            },
            ib_map_dict @ IBencodeType::BencodeMapping(_, Bencode::Dict(_)) => {
                let curr_byte = try!(peek_byte(bytes, pos, "End Of Bytes Encountered"));
                
                if curr_byte == ::BEN_END {
                    // Recursive type has reached it's end, collapse it now
                    pos += 1;
                    let opt_last_type = try!(collapse_bencode(&mut bencode_stack, ib_map_dict, pos));
                    
                    if let Some(last_type) = opt_last_type {
                        return Ok((last_type, pos))
                    }
                } else {
                    // Recursive type still has entries, push it back and then push the entry onto the stack
                    bencode_stack.push(ib_map_dict);
                    
                    let (key, new_pos) = try!(decode_key(bytes, pos));
                    pos = new_pos;
                    
                    let (shallow_type, new_pos) = try!(decode_shallow(bytes, pos));
                    pos = new_pos;
                    
                    bencode_stack.push(IBencodeType::BencodeMapping(key, shallow_type));
                }
            }
        };
    }
    
    panic!("bip_bencode: Reached End Of Decode Without Returning")
}

/// Collapses the given bencode type into the bencode type on the top of the stack.
///
/// Returns Some if there is nothing to collapse the bencode type into.
fn collapse_bencode<'a>(stack: &mut Vec<IBencodeType<'a>>, ibencode: IBencodeType<'a>, curr_pos: usize) -> BencodeParseResult<Option<Bencode<'a>>> {
    if let Some(top_ibencode) = stack.pop() {
        match (top_ibencode, ibencode) {
            (IBencodeType::Bencode(Bencode::Int(_)), _) => {
                panic!("bip_bencode: Attempted To Collapse A Bencode Type Into A Bencode::Int")
            },
            (IBencodeType::Bencode(Bencode::Bytes(_)), _) => {
                panic!("bip_bencode: Attempted To Collapse A Bencode Type Into A Bencode::Bytes")
            },
            (IBencodeType::Bencode(Bencode::List(mut list)), IBencodeType::Bencode(bencode)) => {
                list.push(bencode);
                stack.push(IBencodeType::Bencode(Bencode::List(list)));
                Ok(None)
            },
            (IBencodeType::Bencode(Bencode::List(_)), IBencodeType::BencodeMapping(..)) => {
                panic!("bip_bencode: Attempted To Collapse A Bencode Type Mapping Into A Bencode::List")
            },
            (IBencodeType::Bencode(Bencode::Dict(mut dict)), IBencodeType::BencodeMapping(key, bencode)) => {
                // Spec says that the keys must be in alphabetical order
                match dict.keys().last() {
                    Some(last_key) if key < *last_key => {
                        return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidKey,
                            "Key Not In Alphabetical Order For Dictionary", Some(curr_pos)))
                    },
                    _ => ()
                };
                
                // Spec says that duplicate entries are not allowed
                match dict.entry(key) {
                    Entry::Vacant(n)   => n.insert(bencode),
                    Entry::Occupied(_) => {
                        return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidKey,
                            "Duplicate Key Found For Dictionary", Some(curr_pos)))
                    }
                };
                
                stack.push(IBencodeType::Bencode(Bencode::Dict(dict)));
                Ok(None)
            },
            (IBencodeType::Bencode(Bencode::Dict(_)), IBencodeType::Bencode(_)) => {
                panic!("bip_bencode: Attempted To Collapse A Bencode Type Into A Bencode::Dict")
            },
            (IBencodeType::BencodeMapping(_, Bencode::Int(_)), _) => {
                panic!("bip_bencode: Attempted To Collapse A Bencode Type Into A Bencode::Int Mapping")
            },
            (IBencodeType::BencodeMapping(_, Bencode::Bytes(_)), _) => {
                panic!("bip_bencode: Attempted To Collapse A Bencode Type Into A Bencode::Bytes Mapping")
            },
            (IBencodeType::BencodeMapping(key, Bencode::List(mut list)), IBencodeType::Bencode(bencode)) => {
                list.push(bencode);
                stack.push(IBencodeType::BencodeMapping(key, Bencode::List(list)));
                Ok(None)
            },
            (IBencodeType::BencodeMapping(_, Bencode::List(_)), IBencodeType::BencodeMapping(..)) => {
                panic!("bip_bencode: Attempted To Collapse A Bencode Type Mapping Into A Bencode::List Mapping")
            },
            (IBencodeType::BencodeMapping(key, Bencode::Dict(mut dict)), IBencodeType::BencodeMapping(i_key, bencode)) => {
                // Spec says that the keys must be in alphabetical order
                match dict.keys().last() {
                    Some(last_key) if i_key < *last_key => {
                        return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidKey,
                            "Key Not In Alphabetical Order For Dictionary", Some(curr_pos)))
                    },
                    _ => ()
                };
                
                // Spec says that duplicate entries are not allowed
                match dict.entry(i_key) {
                    Entry::Vacant(n)   => n.insert(bencode),
                    Entry::Occupied(_) => {
                        return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidKey,
                            "Duplicate Key Found For Dictionary", Some(curr_pos)))
                    }
                };
                
                stack.push(IBencodeType::BencodeMapping(key, Bencode::Dict(dict)));
                Ok(None)
            },
            (IBencodeType::BencodeMapping(_, Bencode::Dict(_)), IBencodeType::Bencode(_)) => {
                panic!("bip_bencode: Attempted To Collapse A Bencode Type Into A Bencode::Dict Mapping")
            }
        }
    } else {
        // We need to return the current ibencode as a complete type
        match ibencode {
            IBencodeType::Bencode(bencode)   => Ok(Some(bencode)),
            IBencodeType::BencodeMapping(..) => {
                panic!("bip_bencode: Got BencodeMapping, Expected Bencode")
            }
        }
    }
}

/// Decodes the next shallow bencode type. Any recursive types will be initialized as empty and ending bytes
/// for those types will not be consumed (even if the type is empty).
///
/// Returns the next shallow bencode type as well as the byte of the next type (or end byte for recursive types).
fn decode_shallow<'a>(bytes: &'a [u8], pos: usize) -> BencodeParseResult<(Bencode<'a>, usize)> {
    let curr_byte = try!(peek_byte(bytes, pos, "End Of Bytes Encountered"));

    match curr_byte {
        ::INT_START  => {
            let (bencode, pos) = try!(decode_int(bytes, pos + 1, ::BEN_END));
            Ok((Bencode::Int(bencode), pos))
        },
        ::LIST_START => {
            Ok((Bencode::List(Vec::new()), pos + 1))
        },
        ::DICT_START => {
            Ok((Bencode::Dict(BTreeMap::new()), pos + 1))
        },
        ::BYTE_LEN_LOW...::BYTE_LEN_HIGH => {
            let (bencode, pos) = try!(decode_bytes(bytes, pos));
            // Include the length digit, don't increment position
            Ok((Bencode::Bytes(bencode), pos))
        },
        _ => Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidByte, 
                 "Unknown Bencode Type Token Found", Some(pos)))
    }
}

/// Return the integer as well as the starting byte of the next type.
fn decode_int(bytes: &[u8], pos: usize, delim: u8) -> BencodeParseResult<(i64, usize)> {
    let (_, begin_decode) = bytes.split_at(pos);
    
    let relative_end_pos = match begin_decode.iter().position(|n| *n == delim) {
        Some(end_pos) => end_pos,
        None          => return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidInt,
                             "No Delimiter Found For Integer/Length", Some(pos)))
    };
    let int_byte_slice = &begin_decode[..relative_end_pos];
    
    if int_byte_slice.len() > 1 {
        // Negative zero is not allowed (this would not be caught when converting)
        if int_byte_slice[0] == b'-' && int_byte_slice[1] == b'0' {
            return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidInt,
                "Illegal Negative Zero For Integer/Length", Some(pos)))
        }
    
        // Zero padding is illegal, and unspecified for key lengths (we disallow both)
        if int_byte_slice[0] == b'0' {
            return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidInt,
                "Illegal Zero Padding For Integer/Length", Some(pos)))
        }
    }
    
    let int_str = match str::from_utf8(int_byte_slice) {
        Ok(n)  => n,
        Err(_) => return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidInt,
                      "Invalid UTF-8 Found For Integer/Length", Some(pos)))
    };
    
    // Position of end of integer type, next byte is the start of the next value
    let absolute_end_pos = pos + relative_end_pos;
    match i64::from_str_radix(int_str, 10) {
        Ok(n)  => Ok((n, absolute_end_pos + 1)),
        Err(_) => Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidInt,
                      "Could Not Convert Integer/Length To i64", Some(pos)))
    }
}

/// Returns the byte reference as well as the starting byte of the next type.
fn decode_bytes<'a>(bytes: &'a [u8], pos: usize) -> BencodeParseResult<(&'a [u8], usize)> {
    let (num_bytes, start_pos) = try!(decode_int(bytes, pos, ::BYTE_LEN_END));

    if num_bytes < 0 {
        return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidLength, 
            "Negative Byte Length Found", Some(pos)))
    } 
    
    // Should be safe to cast to usize (TODO: Check if cast would overflow to provide
    // a more helpful error message, otherwise, parsing will probably fail with an
    // unrelated message).
    let num_bytes = num_bytes as usize;
    
    if num_bytes > bytes[start_pos..].len() {
        return Err(BencodeParseError::with_pos(BencodeParseErrorKind::InvalidLength,
            "Overflow Byte Length Found", Some(pos)))
    }
    
    let end_pos = start_pos + num_bytes;
    Ok((&bytes[start_pos..end_pos], end_pos))
}

/// Returns the key reference as well as the starting byte of the next type.
fn decode_key<'a>(bytes: &'a [u8], pos: usize) -> BencodeParseResult<(&'a [u8], usize)> {
    let (key_bytes, next_pos) = try!(decode_bytes(bytes, pos));
    
    Ok((key_bytes, next_pos))
}

fn peek_byte(bytes: &[u8], pos: usize, err_msg: &'static str) -> BencodeParseResult<u8> {
    bytes.get(pos).map(|n| *n).ok_or( BencodeParseError::new(BencodeParseErrorKind::BytesEmpty, err_msg) )
}

#[cfg(test)]
mod tests {
    use bencode::{Bencode};

    // Positive Cases
    const GENERAL: &'static [u8] = b"d0:12:zero_len_key8:location17:udp://test.com:8011:nested dictd4:listli-500500eee6:numberi500500ee";
    const RECURSION: &'static [u8] = b"lllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllleeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
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
        assert_eq!(ben_dict.lookup("".as_bytes()).unwrap().str().unwrap(), "zero_len_key");
        assert_eq!(ben_dict.lookup("location".as_bytes()).unwrap().str().unwrap(), "udp://test.com:80");
        assert_eq!(ben_dict.lookup("number".as_bytes()).unwrap().int().unwrap(), 500500i64);
        
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
        assert_eq!(dict.lookup("test_key".as_bytes()).unwrap().str().unwrap(), "test_value");
        
        let nested_dict = dict.lookup("test_dict".as_bytes()).unwrap().dict().unwrap();
        assert_eq!(nested_dict.lookup("nested_key".as_bytes()).unwrap().str().unwrap(), "nested_value");
        
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
        assert_eq!(nested_dict.lookup("test_key".as_bytes()).unwrap().str().unwrap(), "test_value");
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