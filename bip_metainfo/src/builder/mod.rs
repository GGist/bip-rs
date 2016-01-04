use std::collections::{BTreeMap};
use std::ffi::{OsStr};
use std::path::{Path};

use bip_bencode::{Bencode};
use bip_util::sha::{self};
use chrono::{UTC};
use url::{Url};
use walkdir::{DirEntry, WalkDir};

use builder::worker::{ResultMessage, MasterMessage};
use error::{ParseResult, ParseError, ParseErrorKind};
use parse::{self};

mod queue;
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

const BALANCED_MAX_PIECES_SIZE:  usize = 30000;
const BALANCED_MIN_PIECE_LENGTH: usize = 512 * 1024;

const FILE_SIZE_MAX_PIECES_SIZE:  usize = 10000;
const FILE_SIZE_MIN_PIECE_LENGTH: usize = 1 * 1024 * 1024;

const TRANSFER_MAX_PIECES_SIZE:  usize = 50000;
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

/// Builder for generating a torrent file for one or more local files.
pub struct MetainfoBuilder<'a> {
    root:         BTreeMap<&'a str, Bencode<'a>>,
    info:         BTreeMap<&'a str, Bencode<'a>>,
    // Stored outside of root as some of the variants need the total
    // file sizes in order for the final piece length to be calculated.
    piece_length: PieceLength
}

impl<'a> MetainfoBuilder<'a> {
    /// Create a MetainfoBuilder with the given main tracker.
    pub fn with_tracker(tracker_url: &'a str) -> ParseResult<MetainfoBuilder<'a>> {
        // Check if the tracker is a valid url
        if is_valid_url(tracker_url) {
            Ok(generate_default_builder(tracker_url))
        } else {
            Err(ParseError::new(ParseErrorKind::InvalidData, "Given Tracker Is Not A Valid URL"))
        }
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
    
    /// Builds the torrent file from a single file using the specified number of threads.
    /// This method WILL block.
    ///
    /// Returns the bytes (contents) of the result torrent file.
    pub fn build_from_file<P>(self, file_path: P, num_threads: usize) -> ParseResult<Vec<u8>>
        where P: AsRef<Path> {
        let num_path_elements = file_path.as_ref().iter().count();
        
        let file_entry = try!(gather_file_entries(file_path));
        // Use path elements minus one as the skip count so that we preserve the last element, the file name.
        let file_metadata = try!(map_files_metadata(file_entry.iter(), num_path_elements - 1));
        
        // Make sure we dont have more than one file and that it has 1 path element (it's file name)
        if file_metadata.len() != 1 || file_metadata[0].1.len() != 1 {
            return Err(ParseError::new(ParseErrorKind::InvalidData, "Did Not Find A Single File At File Path"))
        }
        
        let piece_length = determine_piece_length(file_entry[0].0, self.piece_length);
        let pieces = try!(process_files_pieces(file_entry.into_iter(), piece_length, num_threads));
        
        let (file_length, ref file_name) = file_metadata[0];
        
        // Move the builder elements here so they can borrow from the above data
        let mut root = self.root;
        let mut info = self.info;
        
        // Populate the data calulated here into the info dictionary
        info.insert(parse::PIECE_LENGTH_KEY, ben_int!(piece_length as i64));
        info.insert(parse::PIECES_KEY, ben_bytes!(&pieces));
        info.insert(parse::NAME_KEY, ben_bytes!(&file_name[0]));
        info.insert(parse::LENGTH_KEY, ben_int!(file_length));
        
        // Move the info dictionary into the root dictionary
        root.insert(parse::INFO_KEY, Bencode::Dict(info));
        
        // Return the bencoded root dictionary
        Ok(Bencode::Dict(root).encode())
    }
    
    /// Builds the torrent file from a file directory using the specified number
    /// of threads. This method WILL block.
    ///
    /// Returns the bytes (contents) of the result torrent file.
    pub fn build_from_directory<P>(self, dir_path: P, num_threads: usize) -> ParseResult<Vec<u8>>
        where P: AsRef<Path> {
        let num_path_elements = dir_path.as_ref().iter().count();
        
        let file_entries = try!(gather_file_entries(dir_path.as_ref()));
        let file_metadata = try!(map_files_metadata(file_entries.iter(), num_path_elements));
        
        // Make sure they did not give us the path to a single file, if this is the case, there would be no path elements
        let directory_specified = file_metadata.iter().fold(true, |prev, curr| prev && !curr.1.is_empty());
        if !directory_specified {
            return Err(ParseError::new(ParseErrorKind::InvalidData, "Found A Single File, Not A Directory At The Path Specified"))
        }
        
        let total_file_size = file_entries.iter().fold(0, |prev, curr| prev + curr.0);
        let piece_length = determine_piece_length(total_file_size, self.piece_length);
        let pieces = try!(process_files_pieces(file_entries.into_iter(), piece_length, num_threads));
        
        // Grab the directory name and map our file metadat to the correct structure
        let directory_name = dir_path.as_ref().iter().last().unwrap().to_str().unwrap();
        let bencode_files = Bencode::List(file_metadata.iter().map(|&(len, ref path)| {
            let bencode_path = path.iter().map(|p| ben_bytes!(p)).collect();
            
            ben_map! {
                parse::LENGTH_KEY => ben_int!(len),
                parse::PATH_KEY   => Bencode::List(bencode_path)
            }
        }).collect());
        
        // Move the builder elements here so they can borrow from the above data
        let mut root = self.root;
        let mut info = self.info;
        
        // Populate the data calulated here into the info dictionary
        info.insert(parse::PIECE_LENGTH_KEY, ben_int!(piece_length as i64));
        info.insert(parse::PIECES_KEY, ben_bytes!(&pieces));
        info.insert(parse::NAME_KEY, ben_bytes!(directory_name));
        info.insert(parse::FILES_KEY, bencode_files);
        
        // Move the info dictionary into the root dictionary
        root.insert(parse::INFO_KEY, Bencode::Dict(info));
        
        // Return the bencoded root dictionary
        Ok(Bencode::Dict(root).encode())
    }
    
    /// Sets the main tracker url to the given (unvalidated) url.
    fn set_tracker(mut self, valid_tracker_url: &'a str) -> MetainfoBuilder<'a> {
        self.root.insert(parse::ANNOUNCE_URL_KEY, ben_bytes!(valid_tracker_url));
        
        self
    }
}

//----------------------------------------------------------------------------//

/// Generates a default MetainfoBuilder.
fn generate_default_builder<'a>(valid_tracker_url: &'a str) -> MetainfoBuilder<'a> {
    let builder = MetainfoBuilder{ root: BTreeMap::new(), info: BTreeMap::new(),
        piece_length: PieceLength::OptBalanced };
    let default_creation_date = UTC::now().timestamp();
    
    builder.set_tracker(valid_tracker_url).set_creation_date(default_creation_date)
}

/// True if the given url is valid.
fn is_valid_url(url: &str) -> bool {
    Url::parse(url).is_ok()
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
    
    if pot_piece_length < min_piece_length {
        min_piece_length
    } else {
        pot_piece_length
    }
}

/// Generate a ParseError if the given OsStr is not vaid UTF-8, otherwise return it's str equivalent.
fn os_str_to_str<'a>(os_str: &'a OsStr) -> ParseResult<&'a str> {
    os_str.to_str().ok_or(
        ParseError::new(ParseErrorKind::InvalidData, "Found Path Element That Is Not Valid UTF-8")
    )
}

/// Gathers all file entries and their associated file lengths.
///
/// Returns a non empty list of all files found at the directory.
fn gather_file_entries<P>(dirs_path: P) -> ParseResult<Vec<(u64, DirEntry)>>
    where P: AsRef<Path> {
    let mut file_entries = Vec::new();
    // Iterate over only the files that are recursively discovered
    let walkdir_file_iter = WalkDir::new(dirs_path.as_ref()).into_iter().filter(|r| {
        r.as_ref().ok().map_or(true, |d| d.file_type().is_file())
    });
    
    // For each file, push its dir entrty and length as a file entry
    for res_entry in walkdir_file_iter {
        let entry = try!(res_entry);
        let entry_metadata = try!(entry.metadata());
        
        file_entries.push((entry_metadata.len(), entry));
    }
    
    // Return an error if we found no files
    if file_entries.is_empty() {
        let error_msg = format!("Failed To Find Any Files In {}", dirs_path.as_ref().to_string_lossy());
        Err(ParseError::new(ParseErrorKind::IoError, error_msg))
    } else {
        Ok(file_entries)
    }
}

/// Maps the metadata for all files to their converted lengths and relative path lists.
fn map_files_metadata<'a, I>(file_entries: I, num_paths_skip: usize) -> ParseResult<Vec<(i64, Vec<String>)>>
    where I: Iterator<Item=&'a (u64, DirEntry)> {
    let mut files_metadata = Vec::new();
    
    // Iterator over all file entries
    for &(size, ref file) in file_entries {
        // Concat the UTF-8 validated path elements
        let mut paths = Vec::with_capacity(file.path().iter().count());
        for os_str in file.path().into_iter().skip(num_paths_skip) {
            let valid_str = try!(os_str_to_str(os_str));
            
            paths.push(valid_str.to_owned());
        }
        
        // Push the file length and path elements
        files_metadata.push((size as i64, paths));
    }
    
    Ok(files_metadata)
}

/// Concurrently calculates the pieces portion of the given file entries.
fn process_files_pieces<'a, I>(file_entries: I, piece_length: usize, num_threads: usize) -> ParseResult<Vec<u8>>
    where I: Iterator<Item=(u64, DirEntry)> {
    let (send_files, recv_pieces) = worker::start_hasher_workers(piece_length, num_threads);
    
    // Send all the file entries to the master worker (entries of length 0 will fail to mmap)
    for entry in file_entries.filter(|&(size, _)| size != 0).map(|(_, f)| f) {
        send_files.send(MasterMessage::QueueFile(entry)).unwrap();
    }
    
    // Let the master worker know we are don sending file entries
    send_files.send(MasterMessage::ClientFinished).unwrap();
    
    // Receive the result from the master worker
    let hash_pieces = match recv_pieces.recv().unwrap() {
        ResultMessage::Completed(pieces) => pieces,
        ResultMessage::Errored(err)      => return Err(err)
    };
    
    // Concat all of the pieces into a byte string
    let mut concated_pieces = Vec::with_capacity(hash_pieces.len() * sha::SHA_HASH_LEN);
    for hash_piece in hash_pieces.into_iter().map(|(_, h)| h) {
        concated_pieces.extend_from_slice(hash_piece.as_ref());
    }
    
    Ok(concated_pieces)
}