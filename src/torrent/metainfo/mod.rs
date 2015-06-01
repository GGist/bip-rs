//! Different types of torrent file parsers.

use bencode::{BencodeView, EncodeBencode};
use error::{TorrentResult, TorrentErrorKind, TorrentError};
use info_hash::{self, InfoHash};
use torrent::{self};
use util::{self};

mod metainfo;
mod lazy_metainfo;

pub use self::metainfo::{Metainfo};
pub use self::lazy_metainfo::{LazyMetainfo};

// Refers To The Root Metainfo Dictionary (Only Used For Error Messages)
const ROOT_IDENT: &'static str = "root";

// Root Dictionary Keys
const ANNOUNCE_KEY:      &'static str = "announce";
const NODES_KEY:         &'static str = "nodes";
const COMMENT_KEY:       &'static str = "comment";
const CREATED_BY_KEY:    &'static str = "created by";
const CREATION_DATE_KEY: &'static str = "creation date";
const INFO_KEY:          &'static str = "info";

// Info Dictionary Keys
const PRIVATE_KEY:      &'static str = "private";
const LENGTH_KEY:       &'static str = "length";
const MD5SUM_KEY:       &'static str = "md5sum";
const NAME_KEY:         &'static str = "name";
const PATH_KEY:         &'static str = "path";
const PIECE_LENGTH_KEY: &'static str = "piece length";
const PIECES_KEY:       &'static str = "pieces";

// Multi-File Info Dictionary Key
const FILES_KEY: &'static str = "files";

// Length Checks
const MD5SUM_LEN: usize = 32;
const NODE_LEN:   usize = 2;

/// Generate an InfoHash from the given BencodeView value.
fn generate_info_hash<T>(root: &T) -> TorrentResult<InfoHash>
    where T: BencodeView<InnerItem=T> {
    let mut dest_bytes = [0u8; info_hash::INFO_HASH_LEN];
    let root_dict = try!(lazy_metainfo::slice_root_dict(root));
    
    let info = try!(root_dict.lookup(INFO_KEY).ok_or(
        TorrentError::new(TorrentErrorKind::MissingKey, INFO_KEY)
    ));
    let info_bytes = info.encode();
    
    util::apply_sha1(&info_bytes[..], &mut dest_bytes[..]);
    
    Ok(dest_bytes.into())
}