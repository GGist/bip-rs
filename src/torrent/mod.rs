//! Torrent file parsing and validation.

//pub mod extension;
pub mod metainfo;

const PIECE_HASH_LEN: usize = 20;
const INFO_HASH_LEN:  usize = 20;

/// Hash of the Info dictionary of a torrent file.
pub type InfoHash = [u8; INFO_HASH_LEN];

/// Contact information for a remote node.
pub type Node<'a> = (&'a str, u16);

/// Main tracker specified in a torrent file.
pub type MainTracker<'a> = &'a str;

//----------------------------------------------------------------------------//

/// Enumerates different methods of gathering peers.
pub enum ContactType<'a> {
    /// Corresponds To A Torrent File With An "announce" Key.
    Tracker(MainTracker<'a>),
    /// Corresponds To A Torrent File With A "nodes" Key.
    Trackerless(Nodes<'a>),
    /// Corresponds To A Torrent File With An "announce" and "nodes" Key.
    Either(MainTracker<'a>, Nodes<'a>),
    /// Corresponds To A Torrent File With No Contact Type.
    None
}

/// Iterator over remote node contact information.
pub struct Nodes<'a> {
    iter: Box<Iterator<Item=Node<'a>> + 'a>
}

impl<'a> Iterator for Nodes<'a> {
    type Item = Node<'a>;
    
    fn next(&mut self) -> Option<Node<'a>> {
        self.iter.next()
    }
}

//----------------------------------------------------------------------------//

/// Trait for accessing information wtihin a torrent file.
pub trait TorrentView {
    /// Contact method for finding peers for the torrent file. 
    fn contact<'a>(&'a self) -> ContactType<'a>;
    
    /// Comment tag of the current torrent file.
    fn comment(&self) -> Option<&str>;
    
    /// Created by tag of the current torrent file.
    fn created_by(&self) -> Option<&str>;
    
    /// Creation date of the current torrent file in UNIX epoch format.
    fn creation_date(&self) -> Option<i64>;

    /// Piece information for the torrent file.
    fn piece_info<'a>(&'a self) -> PieceInfo<'a>;
    
    /// Iterator over each file within the torrent file.
    fn file_info<'a>(&'a self) -> FileInfo<'a>;
    
    /// SHA-1 hash of the bencoded Info dictionary.
    fn info_hash(&self) -> InfoHash;
}

impl<'b, T> TorrentView for &'b T where T: TorrentView {
    fn contact<'a>(&'a self) -> ContactType<'a> {
        TorrentView::contact(*self)
    }
    
    fn comment(&self) -> Option<&str> {
        TorrentView::comment(*self)
    }
    
    fn created_by(&self) -> Option<&str> {
        TorrentView::created_by(*self)
    }
    
    fn creation_date(&self) -> Option<i64> {
        TorrentView::creation_date(*self)
    }

    fn piece_info<'a>(&'a self) -> PieceInfo<'a> {
        TorrentView::piece_info(*self)
    }
    
    fn file_info<'a>(&'a self) -> FileInfo<'a> {
        TorrentView::file_info(*self)
    }
    
    fn info_hash(&self) -> InfoHash {
        TorrentView::info_hash(*self)
    }
}

impl<'b, T> TorrentView for &'b mut T where T: TorrentView {
    fn contact<'a>(&'a self) -> ContactType<'a> {
        TorrentView::contact(*self)
    }
    
    fn comment(&self) -> Option<&str> {
        TorrentView::comment(*self)
    }
    
    fn created_by(&self) -> Option<&str> {
        TorrentView::created_by(*self)
    }
    
    fn creation_date(&self) -> Option<i64> {
        TorrentView::creation_date(*self)
    }

    fn piece_info<'a>(&'a self) -> PieceInfo<'a> {
        TorrentView::piece_info(*self)
    }
    
    fn file_info<'a>(&'a self) -> FileInfo<'a> {
        TorrentView::file_info(*self)
    }
    
    fn info_hash(&self) -> InfoHash {
        TorrentView::info_hash(*self)
    }
}

//----------------------------------------------------------------------------//

/// Piece information for a torrent file.
pub struct PieceInfo<'a> {
    pieces: &'a [u8],
    length: i64
}

impl<'a> PieceInfo<'a> {
    fn new(pieces: &'a [u8], length: i64) -> PieceInfo<'a> {
        PieceInfo{ pieces: pieces, length: length }
    }

    pub fn length(&self) -> i64 {
        self.length
    }
    
    pub fn pieces(&self) -> Pieces<'a> {
        Pieces::new(self.pieces)
    }
}

/// Iterator over each piece hash of a torrent file.
pub struct Pieces<'a> {
    pieces:   &'a [u8],
    position: usize
}

impl<'a> Pieces<'a> {
    fn new(pieces: &'a [u8]) -> Pieces<'a> {
        Pieces{ pieces: pieces, position: 0 }
    }
}

impl<'a> Iterator for Pieces<'a> {
    type Item = &'a [u8];
    
    fn next(&mut self) -> Option<&'a [u8]> {
        if self.position >= self.pieces.len() {
            None
        } else {
            let curr_pos = self.position;
            
            self.position += PIECE_HASH_LEN;
            
            Some(&self.pieces[curr_pos..curr_pos + PIECE_HASH_LEN])
        }
    }
}

//----------------------------------------------------------------------------//

/// File information for a torrent file.
pub struct FileInfo<'a> {
    directory:  Option<&'a str>,
    file_iter: Files<'a>
}

impl<'a> FileInfo<'a> {
    /// The (suggested) base directory for the files to reside in.
    ///
    /// If this method returns None, you are dealing with a single file torrent,
    /// and if it returns Some, you are dealing with a multi file torrent.
    pub fn directory(&self) -> Option<&'a str> {
        self.directory
    }
    
    /// An iterator over all files within this torrent.
    pub fn files(self) -> Files<'a> {
        self.file_iter
    }
}

/// Iterator over each File within a torrent file.
pub struct Files<'a> {
    iter: Box<Iterator<Item=File<'a>> + 'a>
}

impl<'a> Iterator for Files<'a> {
    type Item = File<'a>;
    
    fn next(&mut self) -> Option<File<'a>> {
        self.iter.next()
    }
}

/// Individual file within a torrent file.
pub struct File<'a> {
    length:    i64,
    checksum:  Option<&'a [u8]>,
    path_iter: FilePath<'a>
}

impl<'a> File<'a> {
    /// Length (in bytes) of the current file.
    pub fn length(&self) -> i64 {
        self.length
    }
    
    /// Checksum (Md5Sum) of the current file.
    pub fn checksum(&self) -> Option<&'a [u8]> {
        self.checksum
    }
    
    /// Iterator over all parts of the current file's directory location.
    ///
    /// The last item produced will be the file name for the current file.
    pub fn path(self) -> FilePath<'a> {
        self.path_iter
    }
}

/// Iterator over each path element for a File.
pub struct FilePath<'a> {
    iter: Box<Iterator<Item=&'a str> + 'a>
}

impl<'a> Iterator for FilePath<'a> {
    type Item = &'a str;
    
    fn next(&mut self) -> Option<&'a str> {
        self.iter.next()
    }
}

//----------------------------------------------------------------------------//

/// Create a new InfoHash.
pub fn new_info_hash() -> InfoHash {
    [0u8; INFO_HASH_LEN]
}