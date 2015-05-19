//! Parse Torrent fields from a recursively defined Bencoded object.

#![allow(dead_code)]

use bencode::{Bencoded, Bencodable};
use error::{TorrentError, TorrentResult};
use error::TorrentErrorKind::{self, WrongType, MissingKey, Other};
use hash;
use util::types::{InfoHash, Dictionary};

use std::borrow::{ToOwned};

// Refers To The Root Metainfo Dictionary
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

// Multi File Info Dictionary Key
const FILES_KEY: &'static str = "files";

// Length Checks
const MD5SUM_LEN: usize = 32;
const NODE_LEN:   usize = 2;

/// Slice from a Bencoded object to another type.
///
/// Returns a WrongType error if it cannot convert to the given type.
macro_rules! slice_ben {
    ($ben:ident, $key:expr, $f:expr) => (
        match $f($ben) {
            Some(n) => n,
            None => return Err(TorrentError{ kind: TorrentErrorKind::WrongType, 
                desc: $key, detail: None }
            )
        }
    )
}

/// Slice from a Dictionary object to a converted Bencoded object.
///
/// Returns a MissingKey error if the value is not in the dictionary or a
/// WrongType error if the value cannot be converted from the Bencoded object.
macro_rules! slice_dict {
    ($dict:ident, $key:expr, $f:expr) => (
        match $dict.lookup($key) {
            Some(n) => match $f(n) {
                Some(n) => n,
                None => return Err(TorrentError{ kind: TorrentErrorKind::WrongType, 
                    desc: $key, detail: None })
            },
            None => return Err(TorrentError{ kind: TorrentErrorKind::MissingKey, 
                desc: $key, detail: None })
        }
    );
}

/// Optionally slice from a Dictionary object to a converted Bencoded object.
///
/// Returns a WrongType error if the value cannot be converted from the Bencoded 
/// object.
macro_rules! slice_dict_opt {
    ($dict:ident, $key:expr, $f:expr) => (
        match $dict.lookup($key) {
            Some(n) => match $f(n) {
                Some(n) => Some(n),
                None => return Err(TorrentError{ kind: TorrentErrorKind::WrongType, 
                    desc: $key, detail: None })
            },
            None => None
        }
    );
}

/// Tries to convert the root Bencoded value to a Dictionary.
pub fn slice_root_dict<T>(root: &T) -> TorrentResult<&Dictionary<String, T>>
    where T: Bencoded<Output=T> {
    Ok(slice_ben!(root, ROOT_IDENT, Bencoded::dict))
}

/// Tries to find the Info dictionary given the root Bencoded object.
pub fn slice_info_dict<T>(root: &T) -> TorrentResult<&Dictionary<String, T>>
    where T: Bencoded<Output=T> {
    let root_dict = try!(slice_root_dict(root));
    
    Ok(slice_dict!(root_dict, INFO_KEY, Bencoded::dict))
}

/// Tries to pull out the Announce value from the root Bencoded value.
pub fn slice_announce<T>(root: &T) -> TorrentResult<&str>
    where T: Bencoded<Output=T> {
    let root_dict = try!(slice_root_dict(root));

    Ok(slice_dict!(root_dict, ANNOUNCE_KEY, Bencoded::str))
}

/// Tries to pull out the Nodes value from the root Bencoded value.
pub fn slice_nodes<T>(root: &T) -> TorrentResult<Vec<(&str, u16)>>
    where T: Bencoded<Output=T> {
    let root_dict = try!(slice_root_dict(root));
    let nodes = slice_dict!(root_dict, NODES_KEY, Bencoded::list);
    
    let mut nodes_list = Vec::with_capacity(nodes.len());
    for i in nodes {
        let node_tuple = try!(i.list().ok_or(
            TorrentError{ kind: WrongType, desc: NODES_KEY, detail: Some("Node Tuple Wrong Type".to_owned()) }
        ));
        
        if node_tuple.len() != NODE_LEN {
            return Err(TorrentError{ kind: WrongType, desc: NODES_KEY, detail: Some("Node Tuple Wrong Size".to_owned()) })
        }
        
        let host = try!(node_tuple[0].str().ok_or(
            TorrentError{ kind: WrongType, desc: NODES_KEY, detail: Some("Host Wrong Type".to_owned()) }
        ));
        let port = try!(node_tuple[1].int().ok_or(
            TorrentError{ kind: WrongType, desc: NODES_KEY, detail: Some("Port Wrong Type".to_owned()) }
        )) as u16;
        
        nodes_list.push((host, port));
    }
    
    Ok(nodes_list)
}

/// Tries to pull out the Comment value from the root Bencoded value.
pub fn slice_comment<T>(root: &T) -> TorrentResult<Option<&str>>
    where T: Bencoded<Output=T> {
    let root_dict = try!(slice_root_dict(root));

    Ok(slice_dict_opt!(root_dict, COMMENT_KEY, Bencoded::str))
}

/// Tries to pull out the Created By value from the root Bencoded value.
pub fn slice_created_by<T>(root: &T) -> TorrentResult<Option<&str>>
    where T: Bencoded<Output=T> {
    let root_dict = try!(slice_root_dict(root));

    Ok(slice_dict_opt!(root_dict, CREATED_BY_KEY, Bencoded::str))
}

/// Tries to pull out the Creation Date value from the root Bencoded value.
pub fn slice_creation_date<T>(root: &T) -> TorrentResult<Option<i64>>
    where T: Bencoded<Output=T> {
    let root_dict = try!(slice_root_dict(root));

    Ok(slice_dict_opt!(root_dict, CREATION_DATE_KEY, Bencoded::int))
}

/// Tries to pull out the Piece Length value from the root Bencoded value.
pub fn slice_piece_length<T>(root: &T) -> TorrentResult<i64> 
    where T: Bencoded<Output=T> {
    let info_dict = try!(slice_info_dict(root));
    
    Ok(slice_dict!(info_dict, PIECE_LENGTH_KEY, Bencoded::int))
}

/// Tires to pull out the Pieces value from the root Bencoded value.
pub fn slice_pieces<T>(root: &T) -> TorrentResult<&[u8]> 
    where T: Bencoded<Output=T> {
    let info_dict = try!(slice_info_dict(root));
    
    Ok(slice_dict!(info_dict, PIECES_KEY, Bencoded::bytes))
}

/// Tries to pull out the Name value from the root Bencoded value.
pub fn slice_name<T>(root: &T) -> TorrentResult<&str> 
    where T: Bencoded<Output=T> {
    let info_dict = try!(slice_info_dict(root));
    
    Ok(slice_dict!(info_dict, NAME_KEY, Bencoded::str))
}

/// Tries to pull out the Md5sum value from the given map.
///
/// Returns an error if the checksum is present but of the wrong length.
pub fn slice_checksum<'a, T>(dict: &'a Dictionary<String, T>) -> TorrentResult<Option<&[u8]>>
    where T: Bencoded<Output=T> + 'a {
    let checksum = slice_dict_opt!(dict, MD5SUM_KEY, Bencoded::bytes);
    
    if checksum.is_some() && checksum.unwrap().len() != MD5SUM_LEN {
        Err(TorrentError{ kind: WrongType, desc: MD5SUM_KEY, detail: Some("Checksum Is The Wrong Length".to_owned()) })
    } else {
        Ok(checksum)
    }
}

/// Tires to pull out all files from the root Bencoded value.
///
/// Works for both single and multi file torrents where the name value will always
/// be the first entry in the paths list that is returned.
pub fn slice_files<T>(root: &T) -> TorrentResult<Vec<(i64, Option<&[u8]>, Vec<&str>)>>
    where T: Bencoded<Output=T> {
    let info_dict = try!(slice_info_dict(root));
    
    let mut file_list = Vec::new();
    let name = slice_dict!(info_dict, NAME_KEY, Bencoded::str);
    
    // Single File Or Multi File
    if info_dict.lookup(LENGTH_KEY).is_some() {
        let length = slice_dict!(info_dict, LENGTH_KEY, Bencoded::int);
        let checksum = try!(slice_checksum(info_dict));
        
        file_list.push((length, checksum, vec![name]));
    } else {
        let files = slice_dict!(info_dict, FILES_KEY, Bencoded::list);
        
        for i in files {
            let file = try!(slice_file(i, name));
            
            file_list.push(file);
        }
    }
    
    Ok(file_list)
}

/// Tries to pull out all file fields from the file Bencoded value.
pub fn slice_file<'a: 'c, 'b: 'c, 'c, T>(file: &'a T, name: &'b str) 
    -> TorrentResult<(i64, Option<&'a [u8]>, Vec<&'c str>)> where T: Bencoded<Output=T> {
    let file = try!(file.dict().ok_or(
        TorrentError{ kind: WrongType, desc: FILES_KEY, detail: Some("File Entry Is Not A Dictionary".to_owned()) }
    ));
    
    let length = slice_dict!(file, LENGTH_KEY, Bencoded::int);
    let checksum = try!(slice_checksum(file));
    
    let paths = slice_dict!(file, PATH_KEY, Bencoded::list);
    
    let mut path_list = Vec::with_capacity(paths.len());
    path_list.push(name);
    
    for i in paths {
        let path_entry = try!(i.str().ok_or(
            TorrentError{ kind: WrongType, desc: PATH_KEY, detail: Some("Path Entry Is Not A UTF-8 String".to_owned()) }
        ));
        
        path_list.push(path_entry);
    }
    
    Ok((length, checksum, path_list))
}

/// Generate an InfoHash from the given Bencoded value.
pub fn generate_info_hash<T>(root: &T) -> TorrentResult<InfoHash>
    where T: Bencoded<Output=T> {
    let root_dict = try!(slice_root_dict(root));
    let info = try!(root_dict.lookup(INFO_KEY).ok_or(
        TorrentError{ kind: MissingKey, desc: INFO_KEY, detail: None }
    ));
    
    let mut dest_bytes = [0u8; hash::SHA1_HASH_LEN];
    let info_bytes = info.encode();
    
    hash::apply_sha1(&info_bytes[..], &mut dest_bytes[..]);
    
    let info_hash = try!(InfoHash::from_bytes(&dest_bytes[..]).ok_or(
        TorrentError{ kind: Other, desc: "Failed To Generate InfoHash From Torrent Bencoded", detail: None }
    ));
    
    Ok(info_hash)
}