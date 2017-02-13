#![recursion_limit = "1024"]

//! Library for parsing and converting bencoded data.
//!
//! # Examples
//!
//! Decoding bencoded data:
//!
//! ```rust
//!     extern crate bip_bencode;
//!
//!     use bip_bencode::{Bencode};
//!
//!     fn main() {
//!         let data = b"d12:lucky_numberi7ee";
//!         let bencode = Bencode::decode(data).unwrap();
//!
//!         assert_eq!(7, bencode.dict().unwrap().lookup("lucky_number".as_bytes())
//!             .unwrap().int().unwrap());
//!     }
//! ```
//!
//! Encoding bencoded data:
//!
//! ```rust
//!     #[macro_use]
//!     extern crate bip_bencode;
//!
//!     use bip_bencode::{Bencode};
//!
//!     fn main() {
//!         let message = (ben_map!{
//!             "lucky_number" => ben_int!(7)
//!         }).encode();
//!
//!         assert_eq!(&b"d12:lucky_numberi7ee"[..], &message[..]);
//!     }
//! ```

#[macro_use]
extern crate error_chain;

mod inner;
//mod mutable;
mod reference;
//mod convert;
mod decode;
//mod dictionary;
//mod encode;
mod error;
//pub mod types;

pub use reference::{BencodeRef, BTypeRef, TypeRef};
pub use reference::dict::{BDictRef, DictRef};
pub use reference::list::{BListRef, ListRef};
//pub use bencode::{Bencode, BencodeKind};
//pub use convert::BencodeConvert;
//pub use dictionary::Dictionary;
pub use error::{BencodeParseError, BencodeParseErrorKind, BencodeParseResult};
pub use error::{BencodeConvertError, BencodeConvertErrorKind, BencodeConvertResult};

const BEN_END: u8 = b'e';
const DICT_START: u8 = b'd';
const LIST_START: u8 = b'l';
const INT_START: u8 = b'i';

const BYTE_LEN_LOW: u8 = b'0';
const BYTE_LEN_HIGH: u8 = b'9';
const BYTE_LEN_END: u8 = b':';
/*
/// Construct a Bencode map by supplying string references as keys and Bencode as values.
#[macro_export]
macro_rules! ben_map {
( $($key:expr => $val:expr),* ) => {
        {
            use std::convert::{AsRef};
            use std::collections::{BTreeMap};
            use bip_bencode::{Bencode};
            
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
            use bip_bencode::{Bencode};
            
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
            use bip_bencode::{Bencode};
            
            Bencode::Bytes(AsRef::as_ref($ben))
        }
    }
}

/// Construct a Bencode integer by supplying an i64.
#[macro_export]
macro_rules! ben_int {
    ( $ben:expr ) => {
        {
            use bip_bencode::{Bencode};
            
            Bencode::Int($ben)
        }
    }
}
*/