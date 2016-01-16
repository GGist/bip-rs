use std::collections::{BTreeMap};
use std::iter::{ExactSizeIterator};

use bip_bencode::{Bencode};
use bip_util::sha::{self, ShaHash};
use chrono::{UTC};

use accessor::{Accessor, IntoAccessor};
use error::{ParseResult};
use parse::{self};

mod buffer;
mod worker;

// Piece length is inversly related to the file size.
// Transfer reliability is inversly related to the piece length.
// Transfer reliability is directly related to the file size.

// These statements hold even today, although the piece lengths that were historically
// recommended may be out of date as we get faster and more reliable network speeds.

// So for balanced, file size, and transfer piece length optimizations, calculate the
// minimum piece length we can do to reach the designated pieces size. Then, if that
// piece length is less than the minimum piece length for that optimization, set it equal
// to the minimum. Setting it equal to the minimum (in that case) will increase the piece
// size which will shrink the pieces size which ensures we do not go outside of our max size.
// This ensure we can generate good piece lengths for both large and small files.

// Maximum Piece Length Across The Board, Takes Priority Over Max Pieces Sizes
// (Not Applied To Custom Lengths)
const ALL_OPT_MAX_PIECE_LENGTH: usize = 16 * 1024 * 1024;

const BALANCED_MAX_PIECES_SIZE:  usize = 40000;
const BALANCED_MIN_PIECE_LENGTH: usize = 512 * 1024;

const FILE_SIZE_MAX_PIECES_SIZE:  usize = 20000;
const FILE_SIZE_MIN_PIECE_LENGTH: usize = 1 * 1024 * 1024;

const TRANSFER_MAX_PIECES_SIZE:  usize = 60000;
const TRANSFER_MIN_PIECE_LENGTH: usize = 1 * 1024;

/// Enumerates settings for piece length for generating a torrent file.
pub enum PieceLength {
    /// Optimize piece length for torrent file size and file transfer.
    OptBalanced,
    /// Optimize piece length for torrent file size.
    OptFileSize,
    /// Optimize piece length for torrent file transfer.
    OptTransfer,
    /// Custom piece length.
    Custom(usize)
}

/// Builder for generating a torrent file from some accessor.
pub struct MetainfoBuilder<'a> {
    root:         BTreeMap<&'a str, Bencode<'a>>,
    info:         BTreeMap<&'a str, Bencode<'a>>,
    // Stored outside of root as some of the variants need the total
    // file sizes in order for the final piece length to be calculated.
    piece_length: PieceLength
}

impl<'a> MetainfoBuilder<'a> {
    /// Create a new MetainfoBuilder with some default values set.
    pub fn new() -> MetainfoBuilder<'a> {
        generate_default_builder()
    }
    
    /// Set the main tracker that this torrent file points to.
    pub fn set_main_tracker(mut self, tracker_url: &'a str) -> MetainfoBuilder<'a> {
        self.root.insert(parse::ANNOUNCE_URL_KEY, ben_bytes!(tracker_url));
        
        self
    }
    
    /// Set the creation date for the torrent.
    ///
    /// Defaults to the current time when the builder was created.
    pub fn set_creation_date(mut self, secs_epoch: i64) -> MetainfoBuilder<'a> {
        self.root.insert(parse::CREATION_DATE_KEY, ben_int!(secs_epoch));
        
        self
    }
    
    /// Set a comment for the torrent file.
    pub fn set_comment(mut self, comment: &'a str) -> MetainfoBuilder<'a> {
        self.root.insert(parse::COMMENT_KEY, ben_bytes!(comment));
        
        self
    }
    
    /// Set the created by for the torrent file.
    pub fn set_created_by(mut self, created_by: &'a str) -> MetainfoBuilder<'a> {
        self.root.insert(parse::CREATED_BY_KEY, ben_bytes!(created_by));
        
        self
    }
    
    /// Sets the private flag for the torrent file.
    pub fn set_private_flag(mut self, is_private: bool) -> MetainfoBuilder<'a> {
        let numeric_is_private = if is_private { 1 } else { 0 };
        self.info.insert(parse::PRIVATE_KEY, ben_int!(numeric_is_private));
        
        self
    }
    
    /// Sets the piece length for the torrent file.
    pub fn set_piece_length(mut self, piece_length: PieceLength) -> MetainfoBuilder<'a> {
        self.piece_length = piece_length;
        
        self
    }
    
    /// Build the metainfo file from the given accessor and the number of worker threads.
    ///
    /// Worker threads are responsible for CPU bound tasks so if IO access is slow, increasing
    /// the number of workers may not be beneficial. This method WILL block until it completes.
    ///
    /// Returns a list of bytes that make up the complete metainfo file.
    pub fn build_as_bytes<A>(self, accessor: A, threads: usize) -> ParseResult<Vec<u8>>
        where A: IntoAccessor {
        let access_owner = try!(accessor.into_accessor());
        
        // Collect all of the file information into a list
        let mut files_info = Vec::new();
        try!(access_owner.access_metadata(|len, path| {
            let path_list: Vec<String> = path.iter().map(|os_str| {
                os_str.to_string_lossy().into_owned()
            }).collect();
            
            files_info.push((len, path_list));
        }));
        
        // Build the pieces for the data our accessor is pointing at
        let total_files_len = files_info.iter().fold(0, |acc, nex| acc + nex.0);
        let piece_length = determine_piece_length(total_files_len, self.piece_length);
        let pieces_list = try!(worker::start_hasher_workers(&access_owner, piece_length, threads));
        let pieces = map_pieces_list(pieces_list.into_iter().map(|(_, piece)| piece));
        
        let mut single_file_name = String::new();
        // Move these here so they are destroyed before the info they borrow
        let mut root = self.root;
        let mut info = self.info;
        
        info.insert(parse::PIECE_LENGTH_KEY, ben_int!(piece_length as i64));
        info.insert(parse::PIECES_KEY, ben_bytes!(&pieces));
        
        // If the accessor specifies a directory OR there are mutliple files, we will build a multi file torrent
        // If the directory is not present but there are multiple files, the direcotry field will be set to empty
        match (access_owner.access_directory(), files_info.len() > 1) {
            (Some(directory), _) => { // Multi File
                let bencode_files = Bencode::List(files_info.iter().map(|&(len, ref path)| {
                    let bencode_path = path.iter().map(|p| ben_bytes!(p)).collect();
                        
                    ben_map! {
                        parse::LENGTH_KEY => ben_int!(len as i64),
                        parse::PATH_KEY   => Bencode::List(bencode_path)
                    }
                }).collect());
                
                info.insert(parse::NAME_KEY, ben_bytes!(directory));
                info.insert(parse::FILES_KEY, bencode_files);
            },
            (None, true)         => { // Multi File
                let bencode_files = Bencode::List(files_info.iter().map(|&(len, ref path)| {
                    let bencode_path = path.iter().map(|p| ben_bytes!(p)).collect();
                        
                    ben_map! {
                        parse::LENGTH_KEY => ben_int!(len as i64),
                        parse::PATH_KEY   => Bencode::List(bencode_path)
                    }
                }).collect());
                
                info.insert(parse::NAME_KEY, ben_bytes!(""));
                info.insert(parse::FILES_KEY, bencode_files);
            },
            (None, false)        => { // Single File
                for name_component in files_info[0].1.iter() {
                    single_file_name.push_str(name_component);
                }
            
                info.insert(parse::LENGTH_KEY, ben_int!(files_info[0].0 as i64));
                info.insert(parse::NAME_KEY, ben_bytes!(&single_file_name));
            }
        }
        // Move the info dictionary into the root dictionary
        root.insert(parse::INFO_KEY, Bencode::Dict(info));
        
        // Return the bencoded root dictionary
        Ok(Bencode::Dict(root).encode())
    }
}

//----------------------------------------------------------------------------//

/// Generates a default MetainfoBuilder.
fn generate_default_builder<'a>() -> MetainfoBuilder<'a> {
    let builder = MetainfoBuilder{ root: BTreeMap::new(), info: BTreeMap::new(),
        piece_length: PieceLength::OptBalanced };
    let default_creation_date = UTC::now().timestamp();
    
    builder.set_creation_date(default_creation_date)
}

/// Calculate the final piece length given the total file size and piece length strategy.
///
/// Lower piece length will result in a bigger file but better transfer reliability and vice versa.
fn determine_piece_length(total_file_size: u64, piece_length: PieceLength) -> usize {
    match piece_length {
        PieceLength::Custom(len) => len,
        PieceLength::OptBalanced => calculate_piece_length(total_file_size, BALANCED_MAX_PIECES_SIZE, BALANCED_MIN_PIECE_LENGTH),
        PieceLength::OptFileSize => calculate_piece_length(total_file_size, FILE_SIZE_MAX_PIECES_SIZE, FILE_SIZE_MIN_PIECE_LENGTH),
        PieceLength::OptTransfer => calculate_piece_length(total_file_size, TRANSFER_MAX_PIECES_SIZE, TRANSFER_MIN_PIECE_LENGTH)
    }
}

/// Calculate the minimum power of 2 piece length for the given max pieces size and total file size.
fn calculate_piece_length(total_file_size: u64, max_pieces_size: usize, min_piece_length: usize) -> usize {
    let num_pieces = (max_pieces_size as f64) / (sha::SHA_HASH_LEN as f64);
    let piece_length = ((total_file_size as f64) / num_pieces + 0.5) as usize;
    
    let pot_piece_length = piece_length.next_power_of_two();
    
    match (pot_piece_length > min_piece_length, pot_piece_length < ALL_OPT_MAX_PIECE_LENGTH) {
        (true, true) => pot_piece_length,
        (false, _)   => min_piece_length,
        (_, false)   => ALL_OPT_MAX_PIECE_LENGTH
    }
}
/// Map the pieces list into a list of bytes (byte string).
fn map_pieces_list<I>(pieces: I) -> Vec<u8> where I: Iterator<Item=ShaHash> + ExactSizeIterator {
    let mut concated_pieces = Vec::with_capacity(pieces.len() * sha::SHA_HASH_LEN);
    for piece in pieces {
        concated_pieces.extend_from_slice(piece.as_ref());
    }
    
    concated_pieces
}