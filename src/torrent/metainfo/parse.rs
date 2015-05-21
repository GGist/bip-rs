//! Parse Torrent fields from a recursively defined BencodeView object.

#![allow(unused)]

use std::borrow::{Cow};

use bencode::{BencodeView, EncodeBencode};
use torrent::{self, InfoHash};
use torrent::error::{TorrentError, TorrentResult, TorrentErrorKind};
use util::{self, Dictionary};

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

/// Slice from a BencodeView object to another type.
///
/// Returns a WrongType error if it cannot convert to the given type.
macro_rules! slice_ben {
    ($ben:ident, $key:expr, $f:expr) => (
        match $f($ben) {
            Some(n) => n,
            None => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
        }
    )
}

/// Slice from a Dictionary object to a converted BencodeView object.
///
/// Returns a MissingKey error if the value is not in the dictionary or a
/// WrongType error if the value cannot be converted from the BencodeView object.
macro_rules! slice_dict {
    ($dict:ident, $key:expr, $f:expr) => (
        match $dict.lookup($key) {
            Some(n) => match $f(n) {
                Some(n) => n,
                None => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
            },
            None => return Err(TorrentError::new(TorrentErrorKind::MissingKey, $key))
        }
    );
}

/// Optionally slice from a Dictionary object to a converted BencodeView object.
///
/// Returns a WrongType error if the value cannot be converted from the BencodeView 
/// object.
macro_rules! slice_dict_opt {
    ($dict:ident, $key:expr, $f:expr) => (
        match $dict.lookup($key) {
            Some(n) => match $f(n) {
                Some(n) => Some(n),
                None => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
            },
            None => None
        }
    );
}

/// Tries to convert the root BencodeView value to a Dictionary.
pub fn slice_root_dict<'a, T>(root: &'a T) -> TorrentResult<&'a Dictionary<String, T>>
    where T: BencodeView<InnerItem=T> {
    Ok(slice_ben!(root, ROOT_IDENT, BencodeView::dict))
}

/// Tries to find the Info dictionary given the root BencodeView object.
pub fn slice_info_dict<'a, T>(root: &'a T) -> TorrentResult<&'a Dictionary<String, T>>
    where T: BencodeView<InnerItem=T> {
    let root_dict = try!(slice_root_dict(root));
    
    Ok(slice_dict!(root_dict, INFO_KEY, BencodeView::dict))
}

/// Tries to pull out the Announce value from the root BencodeView value.
pub fn slice_announce<'a, T>(root: &'a T) -> TorrentResult<&'a str>
    where T: BencodeView<InnerItem=T> {
    let root_dict = try!(slice_root_dict(root));

    Ok(slice_dict!(root_dict, ANNOUNCE_KEY, BencodeView::str))
}

/// Tries to pull out the Nodes value from the root BencodeView value.
pub fn slice_nodes<'a, T>(root: &'a T) -> TorrentResult<Vec<(&'a str, u16)>>
    where T: BencodeView<InnerItem=T> {
    let root_dict = try!(slice_root_dict(root));
    let nodes = slice_dict!(root_dict, NODES_KEY, BencodeView::list);
    
    let mut nodes_list = Vec::with_capacity(nodes.len());
    for i in nodes {
        let node_tuple = try!(i.list().ok_or(
            TorrentError::with_detail(TorrentErrorKind::WrongType, NODES_KEY, "Node Tuple Wrong Type")
        ));
        
        if node_tuple.len() != NODE_LEN {
            return Err(TorrentError::with_detail(TorrentErrorKind::WrongType, NODES_KEY, "Node Tuple Wrong Size"))
        }
        
        let host = try!(node_tuple[0].str().ok_or(
            TorrentError::with_detail(TorrentErrorKind::WrongType, NODES_KEY, "Host Wrong Type")
        ));
        let port = try!(node_tuple[1].int().ok_or(
            TorrentError::with_detail(TorrentErrorKind::WrongType, NODES_KEY, "Port Wrong Type")
        )) as u16;
        
        nodes_list.push((host, port));
    }
    
    Ok(nodes_list)
}

/// Tries to pull out the Comment value from the root BencodeView value.
pub fn slice_comment<'a, T>(root: &'a T) -> TorrentResult<Option<&'a str>>
    where T: BencodeView<InnerItem=T> {
    let root_dict = try!(slice_root_dict(root));

    Ok(slice_dict_opt!(root_dict, COMMENT_KEY, BencodeView::str))
}

/// Tries to pull out the Created By value from the root BencodeView value.
pub fn slice_created_by<'a, T>(root: &'a T) -> TorrentResult<Option<&'a str>>
    where T: BencodeView<InnerItem=T> {
    let root_dict = try!(slice_root_dict(root));

    Ok(slice_dict_opt!(root_dict, CREATED_BY_KEY, BencodeView::str))
}

/// Tries to pull out the Creation Date value from the root BencodeView value.
pub fn slice_creation_date<T>(root: &T) -> TorrentResult<Option<i64>>
    where T: BencodeView<InnerItem=T> {
    let root_dict = try!(slice_root_dict(root));

    Ok(slice_dict_opt!(root_dict, CREATION_DATE_KEY, BencodeView::int))
}

/// Tries to pull out the Piece Length value from the root BencodeView value.
pub fn slice_piece_length<T>(root: &T) -> TorrentResult<i64> 
    where T: BencodeView<InnerItem=T> {
    let info_dict = try!(slice_info_dict(root));
    
    Ok(slice_dict!(info_dict, PIECE_LENGTH_KEY, BencodeView::int))
}

/// Tires to pull out the Pieces value from the root BencodeView value.
pub fn slice_pieces<'a, T>(root: &'a T) -> TorrentResult<&'a [u8]> 
    where T: BencodeView<InnerItem=T> {
    let info_dict = try!(slice_info_dict(root));
    
    Ok(slice_dict!(info_dict, PIECES_KEY, BencodeView::bytes))
}

/// Tries to pull out the Name value from the root BencodeView value.
pub fn slice_name<'a, T>(root: &'a T) -> TorrentResult<&'a str> 
    where T: BencodeView<InnerItem=T> {
    let info_dict = try!(slice_info_dict(root));
    
    Ok(slice_dict!(info_dict, NAME_KEY, BencodeView::str))
}

/// Tries to pull out the md5sum value from the given map.
///
/// Returns an error if the checksum is present but of the wrong length.
pub fn slice_checksum<'a, T>(dict: &'a Dictionary<String, T>) -> TorrentResult<Option<&'a [u8]>>
    where T: BencodeView<InnerItem=T> + 'a {
    let checksum = slice_dict_opt!(dict, MD5SUM_KEY, BencodeView::bytes);
    
    if checksum.is_some() && checksum.unwrap().len() != MD5SUM_LEN {
        Err(TorrentError::with_detail(TorrentErrorKind::WrongType, MD5SUM_KEY, "Checksum Is The Wrong Length"))
    } else {
        Ok(checksum)
    }
}

/// Tires to pull out all files from the root BencodeView value.
///
/// Works for both single and multi file torrents where the name value will always
/// be the first entry in the paths list that is returned.
pub fn slice_files<'a, T>(root: &'a T) -> TorrentResult<Vec<(i64, Option<&'a [u8]>, Vec<&'a str>)>>
    where T: BencodeView<InnerItem=T> {
    let info_dict = try!(slice_info_dict(root));
    
    let mut file_list = Vec::new();
    let name = slice_dict!(info_dict, NAME_KEY, BencodeView::str);
    
    // Single File Or Multi File
    if info_dict.lookup(LENGTH_KEY).is_some() {
        let length = slice_dict!(info_dict, LENGTH_KEY, BencodeView::int);
        let checksum = try!(slice_checksum(info_dict));
        
        file_list.push((length, checksum, vec![name]));
    } else {
        let files = slice_dict!(info_dict, FILES_KEY, BencodeView::list);
        
        for i in files {
            let file = try!(slice_file(i, name));
            
            file_list.push(file);
        }
    }
    
    Ok(file_list)
}

/// Tries to pull out all file fields from the file BencodeView value.
pub fn slice_file<'a: 'c, 'b: 'c, 'c, T>(file: &'a T, name: &'b str) 
    -> TorrentResult<(i64, Option<&'a [u8]>, Vec<&'c str>)>
    where T: BencodeView<InnerItem=T> {
    let file = try!(file.dict().ok_or(
        TorrentError::with_detail(TorrentErrorKind::WrongType, FILES_KEY, "File Entry Is Not A Dictionary")
    ));
    
    let length = slice_dict!(file, LENGTH_KEY, BencodeView::int);
    let checksum = try!(slice_checksum(file));
    
    let paths = slice_dict!(file, PATH_KEY, BencodeView::list);
    
    let mut path_list = Vec::with_capacity(paths.len());
    path_list.push(name);
    
    for i in paths {
        let path_entry = try!(i.str().ok_or(
            TorrentError::with_detail(TorrentErrorKind::WrongType, PATH_KEY, "Path Entry Is Not A UTF-8 String")
        ));
        
        path_list.push(path_entry);
    }
    
    Ok((length, checksum, path_list))
}

/// Generate an InfoHash from the given BencodeView value.
pub fn generate_info_hash<T>(root: &T) -> TorrentResult<InfoHash>
    where T: BencodeView<InnerItem=T> {
    let root_dict = try!(slice_root_dict(root));
    let info = try!(root_dict.lookup(INFO_KEY).ok_or(
        TorrentError::new(TorrentErrorKind::MissingKey, INFO_KEY)
    ));
    let info_bytes = info.encode();
    
    let mut dest_bytes = torrent::create_info_hash();
    util::apply_sha1(&info_bytes[..], &mut dest_bytes[..]);
    
    Ok(dest_bytes)
}