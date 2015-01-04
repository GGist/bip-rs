use bencode::{Bencode};
use error::{TorrErrorKind, TorrError, TorrResult};
use std::collections::{HashMap};

// Metainfo Dictionary Keys
const ANNOUNCE_KEY: &'static str      = "announce";
const ANNOUNCE_LIST_KEY: &'static str = "announce-list";
const COMMENT_KEY: &'static str       = "comment";
const CREATED_BY_KEY: &'static str    = "created by";
const CREATION_DATE_KEY: &'static str = "creation date";
const INFO_KEY: &'static str          = "info";

// Info Dictionary Keys
const LENGTH_KEY: &'static str       = "length";
const MD5SUM_KEY: &'static str       = "md5sum";
const NAME_KEY: &'static str         = "name";
const PATH_KEY: &'static str         = "path";
const PIECE_LENGTH_KEY: &'static str = "piece length";
const PIECES_KEY: &'static str       = "pieces";

// Multi File Info Dictionary Key
const FILES_KEY: &'static str = "files";

const MD5SUM_LENGTH: uint = 32;

/// Used to get a slice of a mandatory torrent value from a dictionary. 
///
/// Returns a TorrError if $key is not present or it's associated value is of 
/// the wrong type.
macro_rules! slice_map(
    ($map:ident, $key:expr, $f:expr) => (
        match $map.get($key) {
            Some(n) => match $f(n) {
                Some(n) => n,
                None => return Err(TorrError{ kind: TorrErrorKind::WrongType, desc: $key, detail: None })
            },
            None => return Err(TorrError{ kind: TorrErrorKind::MissingKey, desc: $key, detail: None })
        }
    );
);

/// Used to get a slice of an optional torrent value from a dictionary.
///
/// Returns a TorrError if $key is present and it's associated value is of the
/// wrong type.
macro_rules! slice_map_opt(
    ($map:ident, $key:expr, $f:expr) => (
        match $map.get($key) {
            Some(n) => match $f(n) {
                Some(n) => Some(n),
                None => return Err(TorrError{ kind: TorrErrorKind::WrongType, desc: $key, detail: None })
            },
            None => None
        }
    );
);

/// A type representing a valid torrent file.
///
/// This type simply stores slices into other types that are actually holding
/// the data. Therefore, it is very cheap to construct and copy around.
pub struct Torrent<'a> {
    announce:      &'a str,
    announce_list: Option<Vec<Vec<&'a str>>>,
    comment:       Option<&'a str>,
    created_by:    Option<&'a str>,
    creation_date: Option<i64>,
    info:          TorrInfo<'a>
}

impl<'a> Torrent<'a> {
    /// Goes through the Bencode object and pulls out the fields required by a
    /// valid torrent file. Any extraneous fields that may be present within
    /// the Bencode object are ignored and will not be considered an error.
    pub fn new(metainfo: &'a Bencode) -> TorrResult<Torrent<'a>> {
        let meta_map = try!(metainfo.dict().ok_or(
            TorrError{ kind: TorrErrorKind::WrongType, desc: "Metainfo Is Not A Dictionary Value", detail: None }
        ));
        
        parse_metainfo(meta_map)
    }
    
    /// Returns the url of the main tracker for the current torrent file.
    pub fn announce(&self) -> &str {
        self.announce
    }
    
    /// Optionally returns a list of urls pointing to backup trackers for the
    /// current torrent file.
    pub fn announce_list(&'a self) -> Option<&'a Vec<Vec<&str>>> {
        match self.announce_list {
            Some(ref n) => Some(n),
            None        => None
        }
    }
    
    /// Optionally returns any comment within the current torrent file.
    pub fn comment(&'a self) -> Option<&'a str> {
        self.comment
    }
    
    /// Optionally returns the created by tag within the current torrent file.
    pub fn created_by(&'a self) -> Option<&'a str> {
        self.created_by
    }
    
    /// Optionally returns the creation date of the current torrent file in 
    /// standard UNIX epoch format.
    ///
    /// Some RFCs say this should be a string, others say it should be an integer.
    /// In practice, all torrents I have seen use an integer so an error will be
    /// generated if it is anything other than an integer.
    pub fn creation_date(&self) -> Option<i64> {
        self.creation_date
    }
    
    /// Returns a tuple of the type of torrent file as well as the associated name.
    /// See below for what the second tuple value represents:
    ///
    /// TorrFileType::Single -> str value will be the name of the file
    /// TorrFIleType::Multi  -> str value will be the name of the root directory
    pub fn file_type(&self) -> (TorrFileType, &str) {
        (self.info.file_type, self.info.name)
    }
    
    /// Returns a list of file objects for the current torrent. 
    ///
    /// If the torrent file is of type TorrFileType::Single, the list will have 
    /// one entry. However, just because it has one entry does not necessarily 
    /// mean it is of type TorrFileType::Single.
    pub fn files(&self) -> &Vec<TorrFile<'a>> {
        &self.info.files
    }
    
    /// Returns the piece length for each file.
    pub fn piece_length(&self) -> i64 {
        self.info.piece_length
    }
    
    /// Returns the pieces byte array for the current torrent.
    pub fn pieces(&self) -> &[u8] {
        self.info.pieces
    }
}

/// Used to transform Vec<Bencode> into Vec<&str> if all Bencode values are of type
/// Bencode::Bytes.
fn get_str_vec<'a>(ben_list: &'a Vec<Bencode>, err_key: &'static str) -> TorrResult<Vec<&'a str>> {
    let mut str_vec = Vec::with_capacity(ben_list.len());
    
    for i in ben_list.iter() {
        let str_ref = try!(i.str().ok_or(
            TorrError{ kind: TorrErrorKind::WrongType, desc: err_key, detail: None }
        ));
        
        str_vec.push(str_ref);
    }
    
    Ok(str_vec)
}

/// Parses the metainfo file, or the root value, of the torrent file.
fn parse_metainfo<'a>(meta_map: &'a HashMap<String, Bencode>) -> TorrResult<Torrent<'a>> {
    let announce = slice_map!(meta_map, ANNOUNCE_KEY, Bencode::str);
    
    let announce_list = slice_map_opt!(meta_map, ANNOUNCE_LIST_KEY, Bencode::list);
    let announce_list = match announce_list {
        Some(n) => {
            let mut outer_list = Vec::with_capacity(n.len());
        
            for i in n.iter() {
                let inner_list = try!(i.list().ok_or(
                    TorrError{ kind: TorrErrorKind::WrongType, desc: ANNOUNCE_LIST_KEY, detail: None }
                ));
                
                outer_list.push(try!(get_str_vec(inner_list, ANNOUNCE_LIST_KEY)));
            }
            
            Some(outer_list)
        },
        None    => None
    };
    
    let comment = slice_map_opt!(meta_map, COMMENT_KEY, Bencode::str);
    let created_by = slice_map_opt!(meta_map, CREATED_BY_KEY, Bencode::str);
    let creation_date = slice_map_opt!(meta_map, CREATION_DATE_KEY, Bencode::int);
    
    let info = try!(TorrInfo::new(slice_map!(meta_map, INFO_KEY, Bencode::dict)));
    
    Ok(Torrent{ announce: announce, announce_list: announce_list, comment: comment,
        created_by: created_by, creation_date: creation_date, info: info })
}

/// Used to represent the type of torrent file.
#[derive(Copy, Show)]
pub enum TorrFileType {
    Single,
    Multi
}

/// Used to represent the info dictionary within the torrent file.
struct TorrInfo<'a> {
    file_type:    TorrFileType,
    files:        Vec<TorrFile<'a>>,
    name:         &'a str,
    piece_length: i64,
    pieces:       &'a [u8]
}

impl<'a> TorrInfo<'a> {
    fn new(info_map: &'a HashMap<String, Bencode>) -> TorrResult<TorrInfo<'a>> {
        let name = slice_map!(info_map, NAME_KEY, Bencode::str);
        let piece_length = slice_map!(info_map, PIECE_LENGTH_KEY, Bencode::int);
        let pieces = slice_map!(info_map, PIECES_KEY, Bencode::bytes);
        
        let file_type;
        let mut files;
        // If the info_map contains FILES_KEY then it is a multi-file torrent
        if info_map.contains_key(FILES_KEY) {
            file_type = TorrFileType::Multi;
            
            let files_ref = slice_map!(info_map, FILES_KEY, Bencode::list);
            files = Vec::with_capacity(files_ref.len());
            
            for i in files_ref.iter() {
                let file_map = try!(i.dict().ok_or(
                    TorrError{ kind: TorrErrorKind::WrongType, desc: FILES_KEY, detail: None }
                ));
                
                files.push(try!(TorrFile::new(file_map, true)));
            }
        } else {
            file_type = TorrFileType::Single;
            
            files = Vec::with_capacity(1);
            
            files.push(try!(TorrFile::new(info_map, false)));
        }
        
        Ok(TorrInfo{ file_type: file_type, files: files, name: name, 
            piece_length: piece_length, pieces: pieces })
    }
}

/// Used to represent a file within a torrent file.
pub struct TorrFile<'a> {
    length: i64,
    md5sum: Option<&'a str>,
    path:   Option<Vec<&'a str>>
}

impl<'a> TorrFile<'a> {
    fn new(file_map: &'a HashMap<String, Bencode>, multi_file: bool) -> TorrResult<TorrFile> {
        let length = slice_map!(file_map, LENGTH_KEY, Bencode::int);
        let md5sum = slice_map_opt!(file_map, MD5SUM_KEY, Bencode::str);
        
        if md5sum.is_some() && md5sum.unwrap().len() != MD5SUM_LENGTH {
            return Err(TorrError{ kind: TorrErrorKind::Other, desc: "Length Of md5sum Is Invalid", detail: None })
        }
        
        let mut path = None;
        if multi_file {
            let path_ref = slice_map!(file_map, PATH_KEY, Bencode::list);
            
            path = Some(try!(get_str_vec(path_ref, PATH_KEY)));
        }
        
        Ok(TorrFile{ length: length, md5sum: md5sum, path: path })
    }
    
    /// Returns the total number of bytes in the current file.
    pub fn length(&self) -> i64 {
        self.length
    }
    
    /// Optionally returns the md5sum of the current file.
    pub fn file_sum(&'a self) -> Option<&'a str> {
        self.md5sum
    }
    
    /// Optionally returns a list of str values that correspond to directories
    /// except for the last value which is the filename for the current file.
    ///
    /// This method returns an Option because for TorrFileType::Single torrents,
    /// the file name is in the root directory.
    pub fn path(&'a self) -> Option<&'a Vec<&str>> {
        match self.path {
            Some(ref n) => Some(n),
            None        => None
        }
    }
}