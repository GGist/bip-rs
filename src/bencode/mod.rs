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
pub enum BencodeKind<'a, T> where T: BencodeView + 'a {
    /// Bencode Integer.
    Int(i64),
    /// Bencode Bytes.
    Bytes(&'a [u8]),
    /// Bencode List.
    List(&'a [T]),
    /// Bencode Dictionary.
    Dict(&'a Dictionary<String, T>)
}

pub trait DecodeBencode<T> {
    fn decode(T) -> BencodeResult<Self>;
}

pub trait EncodeBencode<T> {
    fn encode(self) -> T;
}

impl<T> EncodeBencode<Vec<u8>> for T where T: BencodeView {
    fn encode(self) -> Vec<u8> {
        self::encode::encode(self)
    }
}

/// Trait for viewing the contents of some bencode object.
pub trait BencodeView {
    type InnerItem: BencodeView;

    /// Tries to convert the current value to a str (only valid UTF-8 byte
    /// sequences are convertible).
    fn str(&self) -> Option<&str> {
        match self.bytes() {
            Some(n) => str::from_utf8(n).ok(),
            None    => None
        }
    }
    
    /// The underlying type for the current value.
    fn kind<'a>(&'a self) -> BencodeKind<'a, Self::InnerItem>;
    
    /// Tries to convert the current value to an i64.
    fn int(&self) -> Option<i64>;
    
    /// Tries to convert the current value to a sequence of bytes.
    fn bytes(&self) -> Option<&[u8]>;
    
    /// Tries to convert the current value to a list of InnerItem values.
    fn list(&self) -> Option<&[Self::InnerItem]>;

    /// Tries to convert the current value to a dictionary of InnerItem values.
    fn dict(&self) -> Option<&Dictionary<String, Self::InnerItem>>;
}

impl<'a, T> BencodeView for &'a T where T: BencodeView {
    type InnerItem = <T as BencodeView>::InnerItem;

    fn str(&self) -> Option<&str> {
        BencodeView::str(*self)
    }
    
    fn kind<'b>(&'b self) -> BencodeKind<'b, Self::InnerItem> {
        BencodeView::kind(*self)
    }
    
    fn int(&self) -> Option<i64> {
        BencodeView::int(*self)
    }
    
    fn bytes(&self) -> Option<&[u8]> {
        BencodeView::bytes(*self)
    }
    
    fn list(&self) -> Option<&[Self::InnerItem]> {
        BencodeView::list(*self)
    }

    fn dict(&self) -> Option<&Dictionary<String, Self::InnerItem>> {
        BencodeView::dict(*self)
    }
}

impl<'a, T> BencodeView for &'a mut T where T: BencodeView {
    type InnerItem = <T as BencodeView>::InnerItem;

    fn str(&self) -> Option<&str> {
        BencodeView::str(*self)
    }
    
    fn kind<'b>(&'b self) -> BencodeKind<'b, Self::InnerItem> {
        BencodeView::kind(*self)
    }
    
    fn int(&self) -> Option<i64> {
        BencodeView::int(*self)
    }
    
    fn bytes(&self) -> Option<&[u8]> {
        BencodeView::bytes(*self)
    }
    
    fn list(&self) -> Option<&[Self::InnerItem]> {
        BencodeView::list(*self)
    }

    fn dict(&self) -> Option<&Dictionary<String, Self::InnerItem>> {
        BencodeView::dict(*self)
    }
}