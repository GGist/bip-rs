#[macro_use]
extern crate bip_bencode;
extern crate bip_util;
extern crate chrono;
extern crate memmap;
extern crate url;
extern crate walkdir;

#[cfg(test)]
extern crate rand;

mod builder;
mod error;
mod metainfo;
mod parse;

pub mod iter;

pub use bip_util::bt::{InfoHash};

pub use builder::{MetainfoBuilder, PieceLength};
pub use error::{ParseError, ParseErrorKind, ParseResult};
pub use metainfo::{InfoDictionary, MetainfoFile, File};