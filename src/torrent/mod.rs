//! Accessing fields within a Torrent file.

use bencode::{Bencoded};
use error::{TorrentResult};
use hash;
use util::types::{InfoHash};

pub mod extension;
pub mod metainfo;

mod parse;

/// Different methods for discovering peers for the current torrent.
#[derive(Debug)]
pub enum ContactType<'a> {
    /// Corresponds To A Torrent File With An "announce" Key.
    Tracker(&'a str),
    /// Corresponds To A Torrent File With A "nodes" Key.
    Trackerless(&'a [(&'a str, u16)]),
    /// Corresponds To A Torrent File With An "announce" and "nodes" Key.
    Either(&'a str, &'a [(&'a str, u16)]),
    /// Corresponds To A Torrent File With No Contact Type.
    ///
    /// This can happen when a magnet link does not include any form of contact
    /// in which case it will want us to use the DHT. It can also happen when
    /// a client generates a metainfo file from a magnet link and uses that file
    /// at a later point in time.
    None
}

/// Represents a torrent file that *adheres* to the torrent file specification.
/// 
/// * Currently BEP-0005 makes a distinction between tracker and trackerless
/// metainfo files. Because of it's widespread adoption and in an attempt to
/// make this interface more cohesive, this extension has been incorporated
/// in the form of a contact type.
pub trait Torrent {
    /// Templated for the type of Bencoded representation returned.
    type BencodeType: Bencoded;

    /// Allows for extensions to the bittorrent protocol to expose data.
    ///
    /// Should not be used by clients unless a TorrentExt method has not been provided.
    fn bencode(&self) -> &Self::BencodeType;

    /// Contact method for finding peers for the torrent file. 
    fn contact_type<'a>(&'a self) -> ContactType<'a>;
    
    /// Comment tag of the current torrent file.
    fn comment(&self) -> Option<&str>;
    
    /// Created by tag of the current torrent file.
    fn created_by(&self) -> Option<&str>;
    
    /// Creation date of the current torrent file in standard UNIX epoch format.
    fn creation_date(&self) -> Option<i64>;

    /// Info dictionary of the torrent file.
    fn info<'a>(&'a self) -> &InfoView<'a>;
    
    /// SHA-1 hash of the bencoded Info dictionary.
    fn info_hash(&self) -> &InfoHash;
}

/// The info dictionary of the current torrent.
pub struct InfoView<'a> {
    files:        Vec<FileView<'a>>,
    pieces:       &'a [u8],
    piece_length: i64
}

impl<'a> InfoView<'a> {
    fn new<T>(root: &'a T) -> TorrentResult<InfoView<'a>>
        where T: Bencoded<Output=T> {
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
        let start_index = index * hash::SHA1_HASH_LEN;
        let end_index = start_index + hash::SHA1_HASH_LEN;
        
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