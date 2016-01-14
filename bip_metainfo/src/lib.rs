#[macro_use]
extern crate bip_bencode;
extern crate bip_util;
extern crate chrono;
extern crate crossbeam;
extern crate url;
extern crate walkdir;

#[cfg(test)]
extern crate rand;

mod accessor;
mod builder;
pub mod error;
mod metainfo;
mod parse;

pub mod iter;

pub use bip_util::bt::{InfoHash};

pub use accessor::{Accessor, IntoAccessor, DirectAccessor, FileAccessor};
pub use builder::{MetainfoBuilder, PieceLength};
pub use metainfo::{InfoDictionary, MetainfoFile, File};