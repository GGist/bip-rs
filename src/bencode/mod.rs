//! Bencode parsing and validation.
//!
//! This module exposes a Bencode object that will take in a chunk of bytes and
//! check whether it is valid Bencode or not. It will then allow access to Bencode
//! object by mapping them to data structures for easy manipulation.

use std::convert::{Into};
use std::collections::{HashMap};
use std::io::{Read};
use std::{str};

use error::{BencodeError, BencodeErrorKind, BencodeResult};
use util::{Dictionary};

mod parse;

/// Underlying type reference for the current Bencoded object.
pub enum BencodedKind<'a, T> where T: Bencoded + 'a {
    /// Bencoded Integer.
    Int(i64),
    /// Bencoded Bytes (May Be Convertible To UTF-8).
    Bytes(&'a [u8]),
    /// Bencoded List.
    List(&'a [T]),
    /// Bencoded Dictionary.
    Dict(&'a Dictionary<String, T>)
}

/// Trait for traversing a bencoded object.
pub trait Bencoded {
    type Output: Bencoded;

    /// Tries to convert the current Bencoded value to a str (only valid UTF-8
    /// byte sequences are convertible).
    fn str(&self) -> Option<&str> {
        match self.bytes() {
            Some(n) => str::from_utf8(n).ok(),
            None    => None
        }
    }
    
    /// The underlying type for the current Bencoded value.
    fn kind<'a>(&'a self) -> BencodedKind<'a, Self::Output>;
    
    /// Tries to convert the current Bencoded value to an i64.
    fn int(&self) -> Option<i64>;
    
    /// Tries to convert the current Bencoded value to a sequence of bytes.
    fn bytes(&self) -> Option<&[u8]>;
    
    /// Tries to convert the current Bencoded value to a list of Bencoded values.
    fn list(&self) -> Option<&[Self::Output]>;

    /// Tries to convert the current Bencoded value to a dictionary of Bencoded 
    /// values.
    fn dict(&self) -> Option<&Dictionary<String, Self::Output>>;
}

impl<'a, T> Bencoded for &'a T where T: Bencoded {
    type Output = <T as Bencoded>::Output;

    fn str(&self) -> Option<&str> { Bencoded::str(self) }
    
    fn kind<'b>(&'b self) -> BencodedKind<'b, Self::Output> { Bencoded::kind(self) }
    
    fn int(&self) -> Option<i64> { Bencoded::int(self) }
    
    fn bytes(&self) -> Option<&[u8]> { Bencoded::bytes(self) }
    
    fn list(&self) -> Option<&[Self::Output]> { Bencoded::list(self) }

    fn dict(&self) -> Option<&Dictionary<String, Self::Output>> { Bencoded::dict(self) }
}

impl Into<Vec<u8>> for Bencode  {
    fn into(self) -> Vec<u8> {
        parse::encode(self)
    }
}

/// Structure representing bencoded data.
#[derive(Debug)]
pub enum Bencode {
    /// Bencode Integer.
    Int(i64),
    /// Bencode Bytes (May Be Convertible To UTF-8).
    Bytes(Vec<u8>),
    /// Bencode List.
    List(Vec<Bencode>),
    /// Bencode Dictionary.
    Dict(HashMap<String, Bencode>)
}

impl Bencode {
    /// Processes a series of bytes representing bencoded data.
    ///
    /// Returns a representation of the bencoded file.
    pub fn from_bytes(bytes: &[u8]) -> BencodeResult<Bencode> {
        let mut bytes_iter = bytes.iter().map(|&n| n).enumerate().peekable();

        // Apply try so any errors return before the eof check
        let result = try!(parse::decode(&mut bytes_iter));
        
        if bytes_iter.peek().is_some() {
            return Err(BencodeError::new(BencodeErrorKind::BytesEmpty,
                "End Portion Of bytes Not Processed", None))
        }
        
        Ok(result)
    }
}

impl Bencoded for Bencode {
    type Output = Bencode;

    fn kind<'a>(&'a self) -> BencodedKind<'a, <Self as Bencoded>::Output> {
        match self {
            &Bencode::Int(n)       => BencodedKind::Int(n),
            &Bencode::Bytes(ref n) => BencodedKind::Bytes(n),
            &Bencode::List(ref n)  => BencodedKind::List(n),
            &Bencode::Dict(ref n)  => BencodedKind::Dict(n)
        }
   }
    
    fn int(&self) -> Option<i64> {
        match self {
            &Bencode::Int(n) => Some(n),
            _                => None
        }
    }
    
    fn bytes(&self) -> Option<&[u8]> {
        match self {
            &Bencode::Bytes(ref n) => Some(&n[0..]),
            _                      => None
        }
    }
    
    fn list(&self) -> Option<&[<Self as Bencoded>::Output]> {
    match self {
            &Bencode::List(ref n) => Some(n),
            _                     => None
        }
    }

    fn dict(&self) -> Option<&Dictionary<String, <Self as Bencoded>::Output>> {
        match self {
            &Bencode::Dict(ref n) => Some(n),
            _                     => None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Bencode, Bencoded};

    // Positive Cases
    const GENERAL: &'static [u8] = b"d0:12:zero_len_key8:location17:udp://test.com:8011:nested dictd4:listli-500500eee6:numberi500500ee";
    const BYTES_UTF8: &'static [u8] = b"16:valid_utf8_bytes";

   
    // Negative Cases
    const BYTES_NEG_LEN: &'static [u8] = b"-4:test";
    const BYTES_EXTRA: &'static [u8] = b"l15:processed_bytese17:unprocessed_bytes";
    const BYTES_NOT_UTF8: &'static [u8] = b"5:\xC5\xE6\xBE\xE6\xF2";
   
   #[test]
   fn positive_decode_general() {
        let bencode = Bencode::from_bytes(GENERAL).unwrap();
        
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
        let bencode = Bencode::from_bytes(BYTES_UTF8).unwrap();
        
        assert_eq!(bencode.str().unwrap(), "valid_utf8_bytes");
   }
    
    #[test]
    #[should_panic]
    fn negative_decode_bytes_neg_len() {
        Bencode::from_bytes(BYTES_NEG_LEN).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_bytes_extra() {
        Bencode::from_bytes(BYTES_EXTRA).unwrap();
    }
    
    #[test]
    #[should_panic]
    fn negative_decode_bytes_not_utf8() {
        let bencode = Bencode::from_bytes(BYTES_NOT_UTF8).unwrap();
        
        bencode.str().unwrap();
    }
}