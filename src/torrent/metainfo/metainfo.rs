//! Read a Torrent from a file or some bytes.

use std::fs::{self};
use std::io::{Read};
use std::path::{Path};

use bencode::{Bencode, DecodeBencode};
use error::{TorrentError, TorrentResult, TorrentErrorKind};
use info_hash::{InfoHash};
use torrent::{TorrentView, ContactType, PieceInfo, Files, Nodes,
              FileInfo, File, FilePath};
use torrent::metainfo::{self};
use util::{Dictionary, Iter};

/// Specialized contact for values which may involve owned storage.
///
/// See torrent::ContactType<'a> for client facing interface.
#[derive(Debug, Eq, PartialEq, Clone)]
enum ContactTypeImpl {
    Tracker(String),
    Trackerless(Vec<(String, u16)>),
    Either(String, Vec<(String, u16)>),
    None
}

/// Torrent parser that parses all values during creation.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Metainfo {
    contact:       ContactTypeImpl,
    comment:       Option<String>,
    created_by:    Option<String>,
    creation_date: Option<i64>,
    info:          PieceInfoImpl,
    file:          FileInfoImpl,
    info_hash:     InfoHash
}

impl Metainfo {
    /// Create a new Metainfo object from the given bytes.
    pub fn new<B>(bytes: B) -> TorrentResult<Metainfo> 
        where B: AsRef<[u8]> {
        let mut bencode = try!(Bencode::decode(&bytes));
    
        // Should Be Calculated Before Anything Gets Moved Out
        let info_hash = try!(metainfo::generate_info_hash(&bencode));
    
        let info = try!(PieceInfoImpl::new(&mut bencode));
        let file = try!(FileInfoImpl::new(&mut bencode));
        
        let announce = move_announce(&mut bencode);
        let nodes = move_nodes(&mut bencode);
        let contact = match (announce, nodes) {
            (Ok(a), Ok(n))   => ContactTypeImpl::Either(a, n),
            (Ok(a), Err(_))  => ContactTypeImpl::Tracker(a),
            (Err(_), Ok(n))  => ContactTypeImpl::Trackerless(n),
            (Err(_), Err(_)) => ContactTypeImpl::None
        };
        
        let comment = try!(move_comment(&mut bencode));
        let created_by = try!(move_created_by(&mut bencode));
        let creation_date = try!(move_creation_date(&mut bencode));
        
        Ok(Metainfo{
            contact: contact,
            comment: comment,
            created_by: created_by,
            creation_date: creation_date,
            info: info,
            file: file,
            info_hash: info_hash
        })
    }
    
    /// Create a new Metainfo object from the file located at path.
    pub fn from_file<P>(path: P) -> TorrentResult<Metainfo>
        where P: AsRef<Path> {
        let mut torrent_file = try!(fs::File::open(path));
        let mut torrent_bytes = Vec::new();
        
        try!(torrent_file.read_to_end(&mut torrent_bytes));
        
        Metainfo::new(&torrent_bytes[..])
    }
}

impl TorrentView for Metainfo {
    fn contact<'a>(&'a self) -> ContactType<'a> {
        match self.contact {
            ContactTypeImpl::Tracker(ref tracker) => ContactType::Tracker(tracker),
            ContactTypeImpl::Trackerless(ref nodes) => {
                let iter = Box::new(nodes.iter().map(|&(ref host, port)| {
                    (&host[..], port)
                })) as Box<Iterator<Item=(&'a str, u16)> + 'a>;
                
                ContactType::Trackerless(Nodes{ iter: iter })
            },
            ContactTypeImpl::Either(ref tracker, ref nodes) => {
                let iter = Box::new(nodes.iter().map(|&(ref host, port)| {
                    (&host[..], port)
                })) as Box<Iterator<Item=(&'a str, u16)> + 'a>;
                
                ContactType::Either(tracker, Nodes{ iter: iter })
            },
            ContactTypeImpl::None => ContactType::None
        }
    }
    
    fn comment(&self) -> Option<&str> {
        self.comment.as_ref().map(|n| &n[..])
    }
    
    fn created_by(&self) -> Option<&str> {
        self.created_by.as_ref().map(|n| &n[..])
    }
    
    fn creation_date(&self) -> Option<i64> {
        self.creation_date
    }
    
    fn piece_info<'a>(&'a self) -> PieceInfo<'a> {
        PieceInfo::new(&self.info.pieces[..], self.info.length)
    }
    
    fn file_info<'a>(&'a self) -> FileInfo<'a> {
        self.file.file_info()
    }
    
    fn info_hash(&self) -> InfoHash {
        self.info_hash
    }
}

/// The info dictionary of the current torrent.
#[derive(Debug, Eq, PartialEq, Clone)]
struct PieceInfoImpl {
    pieces: Vec<u8>,
    length: i64
}

impl PieceInfoImpl {
    fn new(root: &mut Bencode) -> TorrentResult<PieceInfoImpl> {
        let length = try!(move_piece_length(root));
        let pieces = try!(move_pieces(root));
        
        Ok(PieceInfoImpl{ pieces: pieces, length: length })
    }
}

/// Type of file which signifies how different fields are interpreted.
/// 
/// We have to make the contents reside inside a tuple because we need to pass
/// a of all the contents to an iterator that maps the contents to the target
/// struct for the client to use.
#[derive(Debug, Eq, PartialEq, Clone)]
enum FileType {
    /// (FileName, Length, Checksum)
    Single((String, i64, Option<Vec<u8>>)),
    /// (BaseDirectory, Files<(Length, Checksum, Path)>)
    Multiple((String, Vec<(i64, Option<Vec<u8>>, Vec<String>)>))
}

#[derive(Debug, Eq, PartialEq, Clone)]
struct FileInfoImpl {
    file_type: FileType
}

impl FileInfoImpl {
    fn new(root: &mut Bencode) -> TorrentResult<FileInfoImpl> {
        Ok(FileInfoImpl{ file_type: try!(move_files(root)) })
    }
    
    fn file_info<'a>(&'a self) -> FileInfo<'a> {
        match self.file_type {
            FileType::Single(ref items) => {
                let file_iter = Box::new(Iter::new(items).map(|&(ref name, len, ref check)| {
                    let path_iter = Box::new(Iter::new(&name[..])) as Box<Iterator<Item=&str>>;
                    
                    File{
                        path_iter: FilePath{ iter: path_iter },
                        length: len,
                        checksum: check.as_ref().map(|n| &n[..])
                    }
                })) as Box<Iterator<Item=File<'a>>>;
                
                FileInfo{
                    directory: None,
                    file_iter: Files{ iter: file_iter }
                }
            },
            FileType::Multiple((ref name, ref items)) => {
                let file_iter = Box::new(items.iter().map(|&(len, ref check, ref path)| {
                    let path_iter = Box::new(path.iter().map(|path| &path[..])) as Box<Iterator<Item=&str>>;
                    
                    File{
                        path_iter: FilePath{ iter: path_iter },
                        length: len,
                        checksum: check.as_ref().map(|n| &n[..])
                    }
                })) as Box<Iterator<Item=File<'a>>>;
                
                FileInfo{
                    directory: Some(name),
                    file_iter: Files{ iter: file_iter }
                }
            }
        }
    }
}

//----------------------------------------------------------------------------//

/// Match a Bencode object to move the inner object.
///
/// Returns a WrongType error if it cannot convert to the given type.
macro_rules! move_ben {
    ($ben:ident, $key:expr, $var:path) => (
        match $ben {
            $var(n) => n,
            _ => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
        }
    )
}

/// Match a Bencode object to get a mutable reference to the inner object.
///
/// Returns a WrongType error if it cannot convert to the given type.
macro_rules! ref_mut_ben {
    ($ben:ident, $key:expr, $var:path) => (
        match *$ben {
            $var(ref mut n) => n,
            _ => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
        }
    )
}

/// Move a Bencode object out of a Dictionary object.
///
/// Returns a MissingKey error if the value is not in the dictionary or a WrongType
/// error if the value cannot be converted object.
macro_rules! remove_dict {
    ($dict:ident, $key:expr, $var:path) => (
        match $dict.remove($key) {
            Some(n) => match n {
                $var(n) => n,
                _ => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
            },
            None => return Err(TorrentError::new(TorrentErrorKind::MissingKey, $key))
        }
    );
}

/// Optionally move a Bencode object out of a Dictionary object.
///
/// Returns a WrongType error if the value cannot be converted object.
macro_rules! remove_dict_opt {
    ($dict:ident, $key:expr, $var:path) => (
        match $dict.remove($key) {
            Some(n) => match n {
                $var(n) => Some(n),
                _ => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
            },
            None => None
        }
    );
}

/// Get a mutable reference to a Bencode object from a Dictionary object.
///
/// Returns a MissingKey error if the value is not in the dictionary or a WrongType
/// error if the value cannot be converted object.
macro_rules! ref_mut_dict {
    ($dict:ident, $key:expr, $var:path) => (
        match $dict.lookup_mut($key) {
            Some(n) => match *n {
                $var(ref mut n) => n,
                _ => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
            },
            None => return Err(TorrentError::new(TorrentErrorKind::MissingKey, $key))
        }
    );
}

//----------------------------------------------------------------------------//

pub fn slice_root_dict<'a, 'b>(root: &'b mut Bencode<'a>) -> TorrentResult<&'b mut Dictionary<'a, Bencode<'a>>> {
    Ok(ref_mut_ben!(root, metainfo::ROOT_IDENT, Bencode::Dict))
}

pub fn slice_info_dict<'a, 'b>(root: &'b mut Bencode<'a>) -> TorrentResult<&'b mut Dictionary<'a, Bencode<'a>>> {
    let root_dict = try!(slice_root_dict(root));
    
    Ok(ref_mut_dict!(root_dict, metainfo::INFO_KEY, Bencode::Dict))
}

pub fn bytes_into_string(bytes: Vec<u8>, key: &'static str) -> TorrentResult<String> {
    String::from_utf8(bytes).map_err(|_|
        TorrentError::with_detail(TorrentErrorKind::WrongType, key, "Bytes Were Not Valid UTF-8")
    )
}

//----------------------------------------------------------------------------//

fn move_announce(root: &mut Bencode) -> TorrentResult<String> {
    let root_dict = try!(slice_root_dict(root));
    let bytes = remove_dict!(root_dict, metainfo::ANNOUNCE_KEY, Bencode::Bytes);
    
    bytes_into_string(bytes.to_vec(), metainfo::ANNOUNCE_KEY)
}

fn move_nodes(root: &mut Bencode) -> TorrentResult<Vec<(String, u16)>> {
    let root_dict = try!(slice_root_dict(root));
    let nodes = remove_dict!(root_dict, metainfo::NODES_KEY, Bencode::List);
    
    let mut nodes_list = Vec::with_capacity(nodes.len());
    for node in nodes {
        let mut node_tuple = move_ben!(node, metainfo::NODES_KEY, Bencode::List);
        
        if node_tuple.len() != metainfo::NODE_LEN {
            return Err(TorrentError::with_detail(TorrentErrorKind::WrongType,
                metainfo::NODES_KEY, "Node Tuple Wrong Size"))
        }
        
        let port_ben = node_tuple.pop().unwrap();
        let host_ben = node_tuple.pop().unwrap();
        
        let port = move_ben!(port_ben, metainfo::NODES_KEY, Bencode::Int) as u16;
        let host = try!(bytes_into_string(
            move_ben!(host_ben, metainfo::NODES_KEY, Bencode::Bytes).to_vec(),
            metainfo::NODES_KEY
        ));
        
        nodes_list.push((host, port));
    }
    
    Ok(nodes_list)
}

fn move_comment(root: &mut Bencode) -> TorrentResult<Option<String>> {
    let root_dict = try!(slice_root_dict(root));
    
    match remove_dict_opt!(root_dict, metainfo::COMMENT_KEY, Bencode::Bytes) {
        Some(n) => bytes_into_string(n.to_vec(), metainfo::COMMENT_KEY).map(Some),
        None    => Ok(None)
    }
}

fn move_created_by(root: &mut Bencode) -> TorrentResult<Option<String>> {
    let root_dict = try!(slice_root_dict(root));
    
    match remove_dict_opt!(root_dict, metainfo::CREATED_BY_KEY, Bencode::Bytes) {
        Some(n) => bytes_into_string(n.to_vec(), metainfo::CREATED_BY_KEY).map(Some),
        None    => Ok(None)
    }
}

fn move_creation_date(root: &mut Bencode) -> TorrentResult<Option<i64>> {
    let root_dict = try!(slice_root_dict(root));
    
    Ok(remove_dict_opt!(root_dict, metainfo::CREATION_DATE_KEY, Bencode::Int))
}

fn move_piece_length(root: &mut Bencode) -> TorrentResult<i64> {
    let info_dict = try!(slice_info_dict(root));
    
    Ok(remove_dict!(info_dict, metainfo::PIECE_LENGTH_KEY, Bencode::Int))
}

fn move_pieces(root: &mut Bencode) -> TorrentResult<Vec<u8>> {
    let info_dict = try!(slice_info_dict(root));
    
    Ok(remove_dict!(info_dict, metainfo::PIECES_KEY, Bencode::Bytes).to_vec())
}

/// 
///
/// Returns an error if the checksum is present but of the wrong length.
fn move_checksum<'a>(dict: &mut Dictionary<'a, Bencode<'a>>) -> TorrentResult<Option<Vec<u8>>> {
    let checksum = remove_dict_opt!(dict, metainfo::MD5SUM_KEY, Bencode::Bytes);
    
    if checksum.is_some() && checksum.as_ref().unwrap().len() != metainfo::MD5SUM_LEN {
        Err(TorrentError::with_detail(TorrentErrorKind::WrongType, metainfo::MD5SUM_KEY,
            "Checksum Is The Wrong Length"))
    } else {
        Ok(checksum.map(|n| n.to_vec()))
    }
}

fn move_name(root: &mut Bencode) -> TorrentResult<String> {
    let info_dict = try!(slice_info_dict(root));
    let name_bytes = remove_dict!(info_dict, metainfo::NAME_KEY, Bencode::Bytes);
    
    bytes_into_string(name_bytes.to_vec(), metainfo::NAME_KEY)
}

/// 
///
/// Works for both single and multi file torrents where the name value will always
/// be the first entry in the paths list that is returned.
fn move_files(root: &mut Bencode) -> TorrentResult<FileType> {
    let name = try!(move_name(root));
    let info_dict = try!(slice_info_dict(root));
    
    // Length Key Only Exists Within The Info Dictionary In Single File Torrents
    if info_dict.lookup(metainfo::LENGTH_KEY).is_some() {
        let length = remove_dict!(info_dict, metainfo::LENGTH_KEY, Bencode::Int);
        let checksum = try!(move_checksum(info_dict));
        
        Ok(FileType::Single((name, length, checksum)))
    } else {
        let files = remove_dict!(info_dict, metainfo::FILES_KEY, Bencode::List);
        let mut file_list = Vec::with_capacity(files.len());
        
        for mut file in files {
            file_list.push(try!(move_file(&mut file)));
        }
        
        Ok(FileType::Multiple((name, file_list)))
    }
}

/// Tries to pull out all file fields from the file BencodeView value.
fn move_file(file: &mut Bencode) -> TorrentResult<(i64, Option<Vec<u8>>, Vec<String>)> {
    let file = ref_mut_ben!(file, metainfo::FILES_KEY, Bencode::Dict);
    
    let length = remove_dict!(file, metainfo::LENGTH_KEY, Bencode::Int);
    let checksum = try!(move_checksum(file));
    
    let paths = remove_dict!(file, metainfo::PATH_KEY, Bencode::List);
    
    let mut path_list = Vec::with_capacity(paths.len());
    
    for path in paths {
        let path_entry_bytes = move_ben!(path, metainfo::PATH_KEY, Bencode::Bytes);
        
        path_list.push(try!(bytes_into_string(path_entry_bytes.to_vec(), metainfo::PATH_KEY)));
    }
    
    Ok((length, checksum, path_list))
}