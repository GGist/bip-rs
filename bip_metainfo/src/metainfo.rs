//! Accessing the fields of a MetainfoFile.

use std::fs::{self};
use std::path::{self};
use std::io::{Read};

use bip_bencode::{Bencode, Dictionary};
use bip_util::bt::{InfoHash};
use bip_util::sha::{self};
use url::{Url};

use parse::{self};
use error::{ParseError, ParseErrorKind, ParseResult};
use iter::{Paths, Files, Pieces};

/// Information about swarms and file(s) referenced by the torrent file.
#[derive(Debug)]
pub struct MetainfoFile {
    comment:         Option<String>,
    announce:        Url,
    encoding:        Option<String>,
    info_hash:       InfoHash,
    created_by:      Option<String>,
    creation_date:   Option<i64>,
    info_dictionary: InfoDictionary
}

impl MetainfoFile {
    /// Read a MetainfoFile from the given bytes.
    pub fn from_bytes<B>(bytes: B) -> ParseResult<MetainfoFile>
        where B: AsRef<[u8]> {
        let bytes_slice = bytes.as_ref();
        
        parse_from_bytes(bytes_slice)
    }
    
    /// Read a MetainfoFile from the given file.
    pub fn from_file<P>(path: P) -> ParseResult<MetainfoFile>
        where P: AsRef<path::Path> {
        let mut file = try!(fs::File::open(path));
        let file_size = try!(file.metadata()).len();
        
        let mut file_bytes = Vec::with_capacity(file_size as usize);
        try!(file.read_to_end(&mut file_bytes));
        
        MetainfoFile::from_bytes(&file_bytes)
    }
    
    /// InfoHash of the InfoDictionary used to identify swarms of peers exchaning these files.
    pub fn info_hash(&self) -> InfoHash {
        self.info_hash
    }
    
    /// Announce url for the main tracker of the metainfo file.
    pub fn announce_url(&self) -> &Url {
        &self.announce
    }
    
    /// Comment included within the metainfo file.
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_ref().map(|c| &c[..])
    }
    
    /// Person or group that created the metainfo file.
    pub fn created_by(&self) -> Option<&str> {
        self.created_by.as_ref().map(|c| &c[..])
    }
    
    /// String encoding format of the peices portion of the info dictionary.
    pub fn encoding(&self) -> Option<&str> {
        self.encoding.as_ref().map(|e| &e[..])
    }
    
    /// Creation date in UNIX epoch format for the metainfo file.
    pub fn creation_date(&self) -> Option<i64> {
        self.creation_date
    }
    
    /// InfoDictionary for the metainfo file.
    pub fn info(&self) -> &InfoDictionary {
        &self.info_dictionary
    }
}

/// Parses the given bytes and builds a MetainfoFile from them.
fn parse_from_bytes(bytes: &[u8]) -> ParseResult<MetainfoFile> {
    let root_bencode = try!(Bencode::decode(bytes).map_err(|_| {
        ParseError::new(ParseErrorKind::CorruptData, "Specified File Is Not Valid Bencode")
    }));
    let root_dict = try!(parse::parse_root_dict(&root_bencode));
    
    let announce = try!(parse::parse_announce_url(root_dict)).to_owned();
    let opt_comment = parse::parse_comment(root_dict).map(|e| e.to_owned());
    let opt_encoding = parse::parse_encoding(root_dict).map(|e| e.to_owned());
    let opt_created_by = parse::parse_created_by(root_dict).map(|e| e.to_owned());
    let opt_creation_date = parse::parse_creation_date(root_dict);
    
    let info_hash = try!(parse::parse_info_hash(root_dict));
    let info_dict = try!(parse::parse_info_dict(root_dict));
    let info_dictionary = try!(InfoDictionary::new(info_dict));
    
    Ok(MetainfoFile{ comment: opt_comment, announce: announce, encoding: opt_encoding, info_hash: info_hash,
        created_by: opt_created_by, creation_date: opt_creation_date, info_dictionary: info_dictionary })
}

//----------------------------------------------------------------------------//

/// Information about the file(s) referenced by the torrent file.
#[derive(Debug)]
pub struct InfoDictionary {
    files:          Vec<File>,
    pieces:         Vec<[u8; sha::SHA_HASH_LEN]>,
    piece_len:      i64,
    is_private:     bool,
    // Present only for multi file torrents.
    file_directory: Option<String>
}

impl InfoDictionary {
    /// Builds the InfoDictionary from the root bencode of the metainfo file.
    fn new<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<InfoDictionary> {
        parse_from_info_dictionary(info_dict)
    }
    
    /// Some file directory if this is a multi-file torrent, otherwise None.
    ///
    /// If you want to check to see if this is a multi-file torrent, you should
    /// check whether or not this returns Some. Checking the number of files
    /// present is NOT the correct method.
    pub fn directory(&self) -> Option<&str> {
        self.file_directory.as_ref().map(|d| &d[..])
    }
    
    /// Length in bytes of each piece.
    pub fn piece_length(&self) -> i64 {
        self.piece_len
    }
    
    /// Whether or not the torrent is private.
    pub fn is_private(&self) -> bool {
        self.is_private
    }
    
    /// Iterator over each of the pieces SHA-1 hash.
    ///
    /// Ordering of pieces yielded in the iterator is guaranteed to be the order in
    /// which they are found in the torrent file as this is necessary to refer to
    /// pieces by their index to other peers.
    pub fn pieces<'a>(&'a self) -> Pieces<'a> {
        Pieces::new(&self.pieces)
    }
    
    /// Iterator over each file within the torrent file.
    ///
    /// Ordering of files yielded in the iterator is guaranteed to be the order in
    /// which they are found in the torrent file as this is necessary to reconstruct
    /// pieces received from peers.
    pub fn files<'a>(&'a self) -> Files<'a> {
        Files::new(&self.files)
    }
}

/// Parses the given info dictionary and builds an InfoDictionary from it.
fn parse_from_info_dictionary<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<InfoDictionary> {
    let piece_len = try!(parse::parse_piece_length(info_dict));
    let is_private = parse::parse_private(info_dict);
    
    let pieces = try!(parse::parse_pieces(info_dict));
    let piece_buffers = try!(allocate_pieces(pieces));
    
    if is_multi_file_torrent(info_dict) { 
        let file_directory = try!(parse::parse_name(info_dict)).to_owned();
        let files_bencode = try!(parse::parse_files_list(info_dict));
        
        let mut files_list = Vec::with_capacity(files_bencode.len());
        for file_bencode in files_bencode {
            let file_dict = try!(parse::parse_file_dict(file_bencode));
            let file = try!(File::as_multi_file(file_dict));
            
            files_list.push(file);
        }
        
        Ok(InfoDictionary{ files: files_list, pieces: piece_buffers, piece_len: piece_len, is_private: is_private,
            file_directory: Some(file_directory)})
    } else {
        let file = try!(File::as_single_file(info_dict));
        
        Ok(InfoDictionary{ files: vec![file], pieces: piece_buffers, piece_len: piece_len, is_private: is_private,
            file_directory: None})
    }
}

/// Returns whether or not this is a multi file torrent.
fn is_multi_file_torrent<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> bool {
    parse::parse_length(info_dict).is_err()
}

/// Validates and allocates the hash pieces on the heap.
fn allocate_pieces(pieces: &[u8]) -> ParseResult<Vec<[u8; sha::SHA_HASH_LEN]>> {
    if pieces.len() % sha::SHA_HASH_LEN != 0 {
        let error_msg = format!("Piece Hash Length Of {} Is Invalid", pieces.len());
        Err(ParseError::new(ParseErrorKind::MissingData, error_msg))
    } else {
        let mut hash_buffers = Vec::with_capacity(pieces.len() / sha::SHA_HASH_LEN);
        let mut hash_bytes = [0u8; sha::SHA_HASH_LEN];
        
        for chunk in pieces.chunks(sha::SHA_HASH_LEN) {
            for (src, dst) in chunk.iter().zip(hash_bytes.iter_mut()) {
                *dst = *src;
            }
            
            hash_buffers.push(hash_bytes);
        }
        
        Ok(hash_buffers)
    }
}

//----------------------------------------------------------------------------//

/// Information about a single file within an InfoDictionary.
#[derive(Debug)]
pub struct File {
    len:    i64,
    path:   Vec<String>,
    md5sum: Option<Vec<u8>>
}

impl File {
    /// Parse the info dictionary and generate a single file File.
    fn as_single_file<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<File> {
        let length = try!(parse::parse_length(info_dict));
        let md5sum = parse::parse_md5sum(info_dict).map(|m| m.to_owned());
        let name = try!(parse::parse_name(info_dict));
        
        Ok(File{ len: length, path: vec![name.to_owned()], md5sum: md5sum })
    }
    
    /// Parse the file dictionary and generate a multi file File.
    fn as_multi_file<'a>(file_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<File> {
        let length = try!(parse::parse_length(file_dict));
        let md5sum = parse::parse_md5sum(file_dict).map(|m| m.to_owned());
        
        let path_list_bencode = try!(parse::parse_path_list(file_dict));
        
        let mut path_list = Vec::with_capacity(path_list_bencode.len());
        for path_bencode in path_list_bencode {
            let path = try!(parse::parse_path_str(path_bencode));
            
            path_list.push(path.to_owned());
        }
        
        Ok(File{ len: length, path: path_list, md5sum: md5sum })
    }
    
    /// Length of the file in bytes.
    pub fn length(&self) -> i64 {
        self.len
    }
    
    /// Optional md5sum of the file.
    ///
    /// Not used by bittorrent.
    pub fn md5sum(&self) -> Option<&[u8]> {
        self.md5sum.as_ref().map(|m| &m[..])
    }
    
    /// Iterator over the path elements of the file.
    ///
    /// The last element is the name of the file.
    pub fn paths<'a>(&'a self) -> Paths<'a> {
        Paths::new(&self.path)
    }
}