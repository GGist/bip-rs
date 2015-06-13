//! Bencode parsing and validation.

use std::{str};

use error::{BencodeResult};
use util::{Dictionary};

mod decode;
mod encode;

const BEN_END:    u8 = b'e';
const DICT_START: u8 = b'd';
const LIST_START: u8 = b'l';
const INT_START:  u8 = b'i';

const BYTE_LEN_LOW:  u8 = b'0';
const BYTE_LEN_HIGH: u8 = b'9';
const BYTE_LEN_END:  u8 = b':';

pub use self::decode::{Bencode};

/// Represents an abstraction into the contents of a BencodeView.
pub enum BencodeKind<'b, 'a: 'b, T> where T: BencodeView<'a> + 'a {
    /// Bencode Integer.
    Int(i64),
    /// Bencode Bytes.
    Bytes(&'a [u8]),
    /// Bencode List.
    List(&'b [T]),
    /// Bencode Dictionary.
    Dict(&'b Dictionary<'a, T>)
}

pub trait DecodeBencode<T> {
    fn decode(T) -> BencodeResult<Self>;
}

pub trait EncodeBencode<T> {
    fn encode(self) -> T;
}

impl<'a, T> EncodeBencode<Vec<u8>> for T where T: BencodeView<'a> {
    fn encode(self) -> Vec<u8> {
        self::encode::encode(self)
    }
}

/// Trait for viewing the contents of some bencode object.
pub trait BencodeView<'a> {
    type InnerView: BencodeView<'a> + 'a;

    /// Tries to convert the current value to a str (only valid UTF-8 byte
    /// sequences are convertible).
    fn str(&self) -> Option<&'a str> {
        match self.bytes() {
            Some(n) => str::from_utf8(n).ok(),
            None    => None
        }
    }
    
    /// The underlying type for the current value.
    fn kind<'b>(&'b self) -> BencodeKind<'b, 'a, Self::InnerView>;
    
    /// Tries to convert the current value to an i64.
    fn int(&self) -> Option<i64>;
    
    /// Tries to convert the current value to a sequence of bytes.
    fn bytes(&self) -> Option<&'a [u8]>;
    
    /// Tries to convert the current value to a list of InnerView values.
    fn list(&self) -> Option<&[Self::InnerView]>;

    /// Tries to convert the current value to a dictionary of InnerView values.
    fn dict(&self) -> Option<&Dictionary<'a, Self::InnerView>>;
}

impl<'a: 'c, 'c, T> BencodeView<'a> for &'c T where T: BencodeView<'a> {
    type InnerView = <T as BencodeView<'a>>::InnerView;

    fn str(&self) -> Option<&'a str> {
        BencodeView::str(*self)
    }
    
    fn kind<'b>(&'b self) -> BencodeKind<'b, 'a, Self::InnerView> {
        BencodeView::kind(*self)
    }
    
    fn int(&self) -> Option<i64> {
        BencodeView::int(*self)
    }
    
    fn bytes(&self) -> Option<&'a [u8]> {
        BencodeView::bytes(*self)
    }
    
    fn list(&self) -> Option<&[Self::InnerView]> {
        BencodeView::list(*self)
    }

    fn dict(&self) -> Option<&Dictionary<'a, Self::InnerView>> {
        BencodeView::dict(*self)
    }
}

impl<'a: 'c, 'c, T> BencodeView<'a> for &'c mut T where T: BencodeView<'a> {
    type InnerView = <T as BencodeView<'a>>::InnerView;

    fn str(&self) -> Option<&'a str> {
        BencodeView::str(*self)
    }
    
    fn kind<'b>(&'b self) -> BencodeKind<'b, 'a, Self::InnerView> {
        BencodeView::kind(*self)
    }
    
    fn int(&self) -> Option<i64> {
        BencodeView::int(*self)
    }
    
    fn bytes(&self) -> Option<&'a [u8]> {
        BencodeView::bytes(*self)
    }
    
    fn list(&self) -> Option<&[Self::InnerView]> {
        BencodeView::list(*self)
    }

    fn dict(&self) -> Option<&Dictionary<'a, Self::InnerView>> {
        BencodeView::dict(*self)
    }
}

mod macros {
    /// Construct a Bencode map by supplying a String keys and Bencode values.
    #[macro_export]
    macro_rules! ben_map {
        ( $($key:expr => $val:expr),* ) => {
            {
                use std::convert::{AsRef};
                use std::collections::{BTreeMap};
                use redox::bencode::{Bencode};
                
                let mut map = BTreeMap::new();
                $(
                    map.insert(AsRef::as_ref($key), $val);
                )*
                Bencode::Dict(map)
            }
        }
    }
    
    /// Construct a Bencode list by supplying a list of Bencode values.
    #[macro_export]
    macro_rules! ben_list {
        ( $($ben:expr),* ) => {
            {
                use redox::bencode::{Bencode};
                
                let mut list = Vec::new();
                $(
                    list.push($ben);
                )*
                Bencode::List(list)
            }
        }
    }
    
    /// Construct Bencode bytes by supplying a type convertible to Vec\<u8\>.
    #[macro_export]
    macro_rules! ben_bytes {
        ( $ben:expr ) => {
            {
                use std::convert::{AsRef};
                use redox::bencode::{Bencode};
                
                Bencode::Bytes(AsRef::as_ref($ben))
            }
        }
    }
    
    /// Construct a Bencode integer by supplying an i64.
    #[macro_export]
    macro_rules! ben_int {
        ( $ben:expr ) => {
            {
                use redox::bencode::{Bencode};
                
                Bencode::Int($ben)
            }
        }
    }
}