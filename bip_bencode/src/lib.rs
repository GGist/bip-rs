//! Library for parsing and converting bencoded data.
//!
//! # Examples
//!
//! Decoding bencoded data:
//!
//! ```rust
//!     extern crate bip_bencode;
//!
//!     use std::default::Default;
//!     use bip_bencode::{BencodeRef, BRefAccess, BDecodeOpt};
//!
//!     fn main() {
//!         let data = b"d12:lucky_numberi7ee";
//!         let bencode = BencodeRef::decode(data, BDecodeOpt::default()).unwrap();
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
//!     fn main() {
//!         let message = (ben_map!{
//!             "lucky_number" => ben_int!(7),
//!             "lucky_string" => ben_bytes!("7")
//!         }).encode();
//!
//!         assert_eq!(&b"d12:lucky_numberi7e12:lucky_string1:7e"[..], &message[..]);
//!     }
//! ```

#[macro_use]
extern crate error_chain;

mod access;
mod cow;
mod mutable;
mod reference;
mod error;

/// Traits for implementation functionality.
pub mod inner {
    pub use cow::BCowConvert;
}

/// Traits for extended functionality.
pub mod ext {
    pub use access::convert::{BConvertExt};
    pub use access::bencode::{BRefAccessExt};
}

pub use reference::bencode_ref::{BencodeRef};
pub use mutable::bencode_mut::{BencodeMut};
pub use access::bencode::{BRefAccess, BencodeRefKind, BMutAccess, BencodeMutKind};
pub use access::convert::{BConvert};
pub use access::dict::BDictAccess;
pub use access::list::BListAccess;
pub use reference::decode_opt::BDecodeOpt;
pub use error::{BencodeParseError, BencodeParseErrorKind, BencodeParseResult};
pub use error::{BencodeConvertError, BencodeConvertErrorKind, BencodeConvertResult};

const BEN_END: u8 = b'e';
const DICT_START: u8 = b'd';
const LIST_START: u8 = b'l';
const INT_START: u8 = b'i';

const BYTE_LEN_LOW: u8 = b'0';
const BYTE_LEN_HIGH: u8 = b'9';
const BYTE_LEN_END: u8 = b':';

/// Construct a `BencodeMut` map by supplying string references as keys and `BencodeMut` as values.
#[macro_export]
macro_rules! ben_map {
( $($key:expr => $val:expr),* ) => {
        {
            use bip_bencode::{BMutAccess, BencodeMut};
            use bip_bencode::inner::BCowConvert;

            let mut bencode_map = BencodeMut::new_dict();
            {
                let mut map = bencode_map.dict_mut().unwrap();
                $(
                    map.insert(BCowConvert::convert($key), $val);
                )*
            }

            bencode_map
        }
    }
}

/// Construct a `BencodeMut` list by supplying a list of `BencodeMut` values.
#[macro_export]
macro_rules! ben_list {
    ( $($ben:expr),* ) => {
        {
            use bip_bencode::{BencodeMut, BMutAccess};
            
            let mut bencode_list = BencodeMut::new_list();
            {
                let mut list = bencode_list.list_mut().unwrap();
                $(
                    list.push($ben);
                )*
            }

            bencode_list
        }
    }
}

/// Construct `BencodeMut` bytes by supplying a type convertible to `Vec<u8>`.
#[macro_export]
macro_rules! ben_bytes {
    ( $ben:expr ) => {
        {
            use bip_bencode::{BencodeMut};
            use bip_bencode::inner::BCowConvert;
            
            BencodeMut::new_bytes(BCowConvert::convert($ben))
        }
    }
}

/// Construct a `BencodeMut` integer by supplying an `i64`.
#[macro_export]
macro_rules! ben_int {
    ( $ben:expr ) => {
        {
            use bip_bencode::{BencodeMut};
            
            BencodeMut::new_int($ben)
        }
    }
}
