use std::fs::{File};
use std::sync::{Arc};
use std::sync::atomic::{Ordering, AtomicUsize};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread::{self};

use bip_util::sha::{ShaHash, ShaHashBuilder};
use walkdir::{DirEntry};
use memmap::{Mmap, Protection};

use error::{ParseError, ParseErrorKind, ParseResult};

/// Messages sent to the master hasher.
pub enum MasterMessage {
    /// This message should only originate from a client!!!
    ///
    /// Process the next file in order.
    QueueFile(DirEntry),
    /// This message should only originate from a client!!!
    ///
    /// This means there are no more files essentially.
    ClientFinished,
    /// This message should only originate from a worker!!!
    ///
    /// Accepts the piece hash with the given piece index.
    AcceptPiece(usize, ShaHash),
    /// This message should only originate from a worker!!!
    ///
    /// This means one of our workers has finished and shut down.
    WorkerFinished
}

/// Message received from the master hasher.
pub enum ResultMessage {
    /// Hashing process completed.
    Completed(Vec<(usize, ShaHash)>),
    /// Hashing process errored out.
    Errored(ParseError)
}

/// Message sent to a worker hasher.
enum WorkerMessage {
    /// Start processing the given memory region.
    ///
    /// Size of the region as well as the region are given.
    HashFile(usize, Arc<Mmap>),
    /// Worker should either exit it's thread or hash
    /// the last (irregular) piece that it was holding on to.
    Finish
}

/// Starts a number of hasher workers which will generate the hash pieces for the files we send to it.
pub fn start_hasher_workers(piece_length: usize, num_workers: usize) -> (Sender<MasterMessage>, Receiver<ResultMessage>) {
    let (master_send, master_recv) = mpsc::channel();
    let (client_send, client_recv) = mpsc::channel();
    
    // Create workers and channels to communicate with them
    let mut worker_senders = Vec::with_capacity(num_workers);
    let piece_index = Arc::new(AtomicUsize::new(0));
    for _ in 0..num_workers {
        let share_piece_index = piece_index.clone();
        let clone_master_send = master_send.clone();
        let (worker_send, worker_recv) = mpsc::channel();
        
        thread::spawn(move || {
            start_hash_worker(clone_master_send, worker_recv, share_piece_index, piece_length);
        });
        
        worker_senders.push(worker_send);
    }
    
    // Create the master worker and channels to communicate with the client_recv
    thread::spawn(move || {
        start_hash_master(num_workers, master_recv, client_send, worker_senders);
    });
    
    (master_send, client_recv)
}

//----------------------------------------------------------------------------//

/// Start a master hasher which will take care of memory mapping files and giving updates to the hasher workers.
fn start_hash_master(num_workers: usize, recv: Receiver<MasterMessage>, send: Sender<ResultMessage>, senders: Vec<Sender<WorkerMessage>>) {
    let mut pieces = Vec::new();
    
    // Loop through all messages until the client initiates a finish.
    for msg in recv.iter() {
        match msg {
            MasterMessage::WorkerFinished             => panic!("bip_metainfo: Worker finished before the client initiated a finish..."),
            MasterMessage::AcceptPiece(index, piece)  => pieces.push((index, piece)),
            MasterMessage::ClientFinished             => { break; },
            MasterMessage::QueueFile(dir_entry)       => {
                // Distribute the file to all of the workers, if an error occured, send it and shut down.
                if let Err(e) = distribute_worker_file(&dir_entry, &senders[..]) {
                    send.send(ResultMessage::Errored(e)).unwrap();
                    return
                }
            }
        }
    }
    
    // At this point, there are no more files to hash. We should let all workers know that they need to finish up.
    for send in senders.iter() {
        send.send(WorkerMessage::Finish).unwrap();
    }
    
    // Wait for all of the workers to either send us the last piece(s) or let us know they are finished.
    let mut workers_finished = 0;
    while workers_finished < num_workers {
        match recv.recv() {
            Ok(MasterMessage::AcceptPiece(index, piece)) => pieces.push((index, piece)),
            Ok(MasterMessage::WorkerFinished)            => workers_finished += 1,
            Ok(MasterMessage::ClientFinished)            => panic!("bip_metainfo: Client told us they finished twice..."),
            Ok(MasterMessage::QueueFile(..))             => panic!("bip_metainfo: Client tried to queue a file after finishing..."),
            Err(_)                                       => panic!("bip_metainfo: Master reciever failed before all workers finished...")
        }
    }
    
    // Sort our list to make sure the pieces are in order before we send them off
    pieces.sort_by(|one, two| {
        one.0.cmp(&two.0)
    });
    
    send.send(ResultMessage::Completed(pieces)).unwrap();
}

/// Distribute the file to the hasher workers.
fn distribute_worker_file(dir_entry: &DirEntry, senders: &[Sender<WorkerMessage>]) -> ParseResult<()> {
    // Try to open the file and mmap it.
    if let Ok(Ok(mmap)) = File::open(dir_entry.path()).map(|f| Mmap::open(&f, Protection::Read)) {
        let mmap = Arc::new(mmap);
        
        // Send the mmap and its size to all workers.
        for send in senders {
            let shared_mmap = mmap.clone();
            send.send(WorkerMessage::HashFile(shared_mmap.len(), shared_mmap)).unwrap();
        }
        
        Ok(())
    } else {
        Err(ParseError::new(ParseErrorKind::IoError, "Failed To Open Or Mmap File"))
    }
}

//----------------------------------------------------------------------------//

/// Saves the state of a partial piece (hash) so it can be re-processed with the next mmap.
///
/// A partial hash state could extend beyond more than two files in the worst (complex) case
/// if one of the files is smaller than the piece length which is definitely possible.
struct PartialHashState {
    /// Accumulates the bytes within the mmaps
    builder:     ShaHashBuilder,
    piece_index: usize,
    /// Numbers of byte left to process from the start of the next mmap.
    bytes_left:  usize
}

/// Starts a hasher worker which will hash all of the mmap handles it receives. Partial pieces will be processed across multiple
/// mmap regions. If a worker receives a finished message, any partial piece will be hashed immediately and yielded.
fn start_hash_worker(send: Sender<MasterMessage>, recv: Receiver<WorkerMessage>, curr_piece_index: Arc<AtomicUsize>, piece_length: usize) {
    let mut previous_mmaps_size = 0;
    let mut opt_partial_hash_state = None;
    
    // Receive messages until the master tells us to finish up.
    for msg in recv.iter() {
        match msg {
            WorkerMessage::HashFile(size, mmap) => {
                // Try to process any partial hashes we saved, if we dont have any saved, process the current mmap itself.
                //
                // The idea here is that we we have a partial piece, we need to process the start of the next mmap up to the amount
                // of bytes we need to make it a whole piece. If the current mmap didnt satisfy that, it means we need to wait for
                // the next mmap to consume more bytes. In that case, dont clear the partial state (dont call process_current_mmap).
                opt_partial_hash_state = opt_partial_hash_state.take().and_then(|p| {
                    process_overlapping_mmap(p, &*mmap, &send)
                }).or_else(|| {
                    process_current_mmap(&*mmap, &curr_piece_index, previous_mmaps_size, piece_length, &send)
                });
                
                // Add up the total file size we have seen so far.
                previous_mmaps_size += size;
            },
            WorkerMessage::Finish => {
                // Check if we still have a partial piece being built, if so, build it and send it.
                opt_partial_hash_state.as_ref().map(|p| {
                    send.send(MasterMessage::AcceptPiece(p.piece_index, p.builder.build())).unwrap();
                });
                
                send.send(MasterMessage::WorkerFinished).unwrap();
            }
        }
    }
}

/// Hashes as much of the mmap as possible given the mmap, the current piece index, and the total number of bytes hashsed
/// in previous mmaps.
///
/// Optionally returns the piece index of the partial piece for the current mmap.
fn process_current_mmap(mmap: &Mmap, curr_piece_index: &Arc<AtomicUsize>, previous_mmaps_size: usize, piece_length: usize,
    send: &Sender<MasterMessage>) -> Option<PartialHashState> {
    let curr_mmap_slice = unsafe{ mmap.as_slice() };
    
    // Grab a piece index
    // TODO: Make sure this is the correct ordering, and that we cant relax it
    let mut next_piece_index = curr_piece_index.fetch_add(1, Ordering::AcqRel);
    // Calculate the piece index offset
    let mut next_mmap_offset = calculate_piece_offset(previous_mmaps_size, next_piece_index, piece_length);
    
    // While the offset of the next piece resides (even partially) in the current mmap
    while next_mmap_offset < curr_mmap_slice.len() {
        let end_mmap_offset = next_mmap_offset + piece_length;
        
        // Check if our piece slice extends into the next mmap
        if end_mmap_offset >= curr_mmap_slice.len() {
            // Grab the end slice for the partial piece
            let partial_slice = &curr_mmap_slice[next_mmap_offset..];
            
            let builder = ShaHashBuilder::new().add_bytes(partial_slice);
            let bytes_left = piece_length - partial_slice.len();
            
            // Return the partial piece state
            return Some(PartialHashState{ builder: builder, piece_index: next_piece_index, bytes_left: bytes_left })
        } else {
            // Hash the complete piece contained in the current mmap
            let hash = ShaHash::from_bytes(&curr_mmap_slice[next_mmap_offset..end_mmap_offset]);
            
            // Send the completed piece hash back to the master
            send.send(MasterMessage::AcceptPiece(next_piece_index, hash)).unwrap();
        }
        
        // Grab a new piece index
        next_piece_index = curr_piece_index.fetch_add(1, Ordering::AcqRel);
        // Calculate the new piece index offset
        next_mmap_offset = calculate_piece_offset(previous_mmaps_size, next_piece_index, piece_length);
    }
    
    None
}

/// Processes the partial piece from one mmap extending onto the next mmap.
///
/// Returns the state back if it needs to be processed with the next call to mmap, or None it is has finished.
fn process_overlapping_mmap(mut prev_state: PartialHashState, curr_mmap: &Mmap, send: &Sender<MasterMessage>) -> Option<PartialHashState> {
    let bytes_left = prev_state.bytes_left;
    let curr_mmap_slice = unsafe{ curr_mmap.as_slice() };
    
    if bytes_left > curr_mmap_slice.len() {
        // We are going to have to process this partial piece again after this
        prev_state.bytes_left -= curr_mmap_slice.len();
        prev_state.builder = prev_state.builder.add_bytes(curr_mmap_slice);
        
        Some(prev_state)
    } else {
        // We are not going to have to process this partial peice again after this
        prev_state.bytes_left = 0;
        let hash = prev_state.builder.add_bytes(&curr_mmap_slice[0..bytes_left]).build();
        
        // Send the processed piece
        send.send(MasterMessage::AcceptPiece(prev_state.piece_index, hash)).unwrap();
        
        None
    }
}

/// Calculates the offset into the current mmap that the given piece index resides at.
fn calculate_piece_offset(previous_mmaps_size: usize, piece_index: usize, piece_length: usize) -> usize {
    // Offset in relation to all previously processed mmaps
    let global_offset = piece_index * piece_length;
    
    // Local offset into the current (or future) mmap
    global_offset - previous_mmaps_size
}