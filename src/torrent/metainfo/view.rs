//! Read a Torrent from a file or some bytes.

use std::borrow::{ToOwned};
use std::error::{Error};
use std::fs::{File};
use std::io::{Read};
use std::mem;
use std::path::{Path};

use bencode::{Bencode, BencodeView, DecodeBencode};
use torrent::{Torrent, ContactType, InfoHash};
use torrent::error::{TorrentError, TorrentResult};
use torrent::parse::{self};
use util::{self};

/// Specialized contact for values which may involve owned storage.
///
/// See torrent::ContactType<'a> for client facing interface.
enum Contact<'a> {
    Tracker(&'a str),
    Trackerless(Vec<(&'a str, u16)>),
    Either(&'a str, Vec<(&'a str, u16)>),
    None
}

pub struct MetainfoView<'a> {
    contact:       Contact<'a>,
    comment:       Option<&'a str>,
    created_by:    Option<&'a str>,
    creation_date: Option<i64>,
    info:          InfoView<'a>,
    info_hash:     InfoHash
}

impl<'a> MetainfoView<'a> {
    pub fn new<T>(bencode: &'a T) -> TorrentResult<MetainfoView<'a>>
        where T: BencodeView<InnerItem=T> {
        let info = try!(InfoView::new(bencode));
        let info_hash = try!(parse::generate_info_hash(bencode));
        
        let announce = parse::slice_announce(bencode);
        let nodes = parse::slice_nodes(bencode);
        let contact = match (announce, nodes) {
            (Ok(a), Ok(n))   => Contact::Either(a, n),
            (Ok(a), Err(_))  => Contact::Tracker(a),
            (Err(_), Ok(n))  => Contact::Trackerless(n),
            (Err(_), Err(_)) => Contact::None
        };
        
        let comment = try!(parse::slice_comment(bencode));
        let created_by = try!(parse::slice_created_by(bencode));
        let creation_date = try!(parse::slice_creation_date(bencode));
        
        Ok(MetainfoView{
            contact: contact,
            comment: comment,
            created_by: created_by,
            creation_date: creation_date,
            info: info,
            info_hash: info_hash
        })
    }
}

impl<'b> Torrent for MetainfoView<'b> {
    fn contact_type<'a>(&'a self) -> ContactType<'a> {
        match self.contact {
            Contact::Tracker(a)         => ContactType::Tracker(a),
            Contact::Trackerless(ref b) => ContactType::Trackerless(&b[..]),
            Contact::Either(a, ref b)   => ContactType::Either(a, &b[..]),
            Contact::None               => ContactType::None
        }
    }
    
    fn comment(&self) -> Option<&str> {
        self.comment
    }
    
    fn created_by(&self) -> Option<&str> {
        self.created_by
    }
    
    fn creation_date(&self) -> Option<i64> {
        self.creation_date
    }   

    fn info<'a>(&'a self) -> &InfoView<'a> {
        &self.info
    }
    
    fn info_hash(&self) -> &InfoHash {
        &self.info_hash
    }
}


/// The info dictionary of the current torrent.
pub struct InfoView<'a> {
    files:        Vec<FileView<'a>>,
    pieces:       &'a [u8],
    piece_length: i64
}

impl<'a> InfoView<'a> {
    fn new<T>(root: &'a T) -> TorrentResult<InfoView<'a>>
        where T: BencodeView<InnerItem=T> {
        let files = try!(parse::slice_files(root));
        
        let file_views = files.into_iter().map(|(len, sum, path)|
            FileView{ length: len, path: path, checksum: sum }
        ).collect();
        
        Ok(InfoView{ files: file_views, 
            pieces: try!(parse::slice_pieces(root)),
            piece_length: try!(parse::slice_piece_length(root))
        })
    }
    
    /// Number of pieces in total.
    ///
    /// Equal to the number of piece hahses in the torrent file.
    pub fn num_pieces(&self) -> usize {
        self.pieces.len()
    }
    
    /// SHA-1 hash for the specified piece.
    ///
    /// Returns None if index is out of bounds (>= self.num_pieces()).
    pub fn piece_hash(&self, index: usize) -> Option<&[u8]> {
        let start_index = index * util::SHA1_HASH_LEN;
        let end_index = start_index + util::SHA1_HASH_LEN;
        
        if end_index >= self.pieces.len() {
            None
        } else {
            Some(&self.pieces[start_index..end_index])
        }
    }

    /// Number of bytes in each piece.
    pub fn piece_length(&self) -> i64 {
        self.piece_length
    }
    
    /// An ordered list of files for the current torrent.
    ///
    /// Single file torrents and multi file torrents have been abstracted over.
    /// See the path method of the File trait for more information.
    pub fn files(&self) -> &[FileView<'a>] {
        &self.files[..]
    }
}

/// A file within the current torrent.
pub struct FileView<'a> {
    length:   i64,
    path:     Vec<&'a str>,
    checksum: Option<&'a [u8]>
}

impl<'a> FileView<'a> {
    /// Size of the file in bytes.
    pub fn file_size(&self) -> i64 {
        self.length
    }
    
    /// MD5 checksum of the file.
    ///
    /// Not used by bittorrent, provided for compatibility with other applications.
    pub fn checksum(&self) -> Option<&[u8]> {
        self.checksum
    }
        
    /// File path of the current file. The last entry will always be the file name.
    ///
    /// The name field typically specified in the info dictionary will instead be
    /// the first entry of each path list regardless of whether or not this is a
    /// multi file or single file torrent.
    /// This makes it easier to abstract the different between a multi and single 
    /// file torrent and provides easier integration with other bittorrent extensions
    /// such as http (web) seeding.
    pub fn path(&self) -> &[&str] {
        &self.path[..]
    }
}