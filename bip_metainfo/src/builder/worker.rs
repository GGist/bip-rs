use std::fs::{File};
use std::sync::{Arc};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread::{self};

use bip_util::sha::{ShaHash, ShaHashBuilder};
use walkdir::{DirEntry};
use memmap::{Mmap, Protection};

use builder::queue::{IndexQueue};
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
    let index_queue = Arc::new(IndexQueue::new());
    for _ in 0..num_workers {
        let share_index_queue = index_queue.clone();
        let clone_master_send = master_send.clone();
        let (worker_send, worker_recv) = mpsc::channel();
        
        thread::spawn(move || {
            start_hash_worker(clone_master_send, worker_recv, share_index_queue, piece_length);
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
fn start_hash_master(num_workers: usize, recv: Receiver<MasterMessage>, send: Sender<ResultMessage>,
    senders: Vec<Sender<WorkerMessage>>) {
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

/// Starts a hasher worker which will hash all of the mmap handles it receives. Partial pieces will be processed across
/// multiple mmap regions. If a worker receives a finished message, any partial piece will be hashed immediately and
/// yielded.
fn start_hash_worker(send: Sender<MasterMessage>, recv: Receiver<WorkerMessage>, index_queue: Arc<IndexQueue>,
    piece_length: usize) {
    let mut region_bytes_processed = 0;
    let mut opt_partial_hash_state = None;
    
    // Receive messages until the master tells us to finish up.
    for msg in recv.iter() {
        match msg {
            WorkerMessage::HashFile(size, mmap) => {
                // Just need to make sure that the underlying mmap is not modified (by us or anyone else).
                let region = unsafe{ mmap.as_slice() };
                
                // Try to process any partial hashes we saved, if we dont have any saved, process the current mmap itself.
                //
                // The idea here is that we we have a partial piece, we need to process the start of the next mmap up to the amount
                // of bytes we need to make it a whole piece. If the current mmap didnt satisfy that, it means we need to wait for
                // the next mmap to consume more bytes. In that case, dont clear the partial state (dont call process_current_mmap).
                opt_partial_hash_state = opt_partial_hash_state.take().and_then(|p| {
                    process_overlapping_region(p, region, &send)
                }).or_else(|| {
                    process_region(region, &index_queue, region_bytes_processed, piece_length, &send)
                });
                
                // Add up the total file size we have seen so far.
                region_bytes_processed += size;
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

/// Hashes as much of the region as possible at the current piece index with the total number of bytes hashsed in
/// previous regions.
///
/// Optionally returns the piece index of the partial piece for the current region.
fn process_region(region: &[u8], index_queue: &Arc<IndexQueue>, region_bytes_processed: usize, piece_length: usize,
    send: &Sender<MasterMessage>) -> Option<PartialHashState> {
    // Grab a piece index
    // TODO: Make sure this is the correct ordering, and that we cant relax it
    let mut next_piece_index = index_queue.get();
    // Calculate the piece index offset
    let mut next_region_offset = calculate_piece_offset(region_bytes_processed, next_piece_index.get(), piece_length);
    
    // While the offset of the next piece resides (even partially) in the current region
    while next_region_offset < region.len() {
        let end_region_offset = next_region_offset + piece_length;
        
        // Check if our piece slice extends into the next mmap
        if end_region_offset > region.len() {
            // Grab the end slice for the partial piece
            let partial_slice = &region[next_region_offset..];
            
            let builder = ShaHashBuilder::new().add_bytes(partial_slice);
            let bytes_left = piece_length - partial_slice.len();
            
            // Return the partial piece state
            return Some(PartialHashState{ builder: builder, piece_index: next_piece_index.get(), bytes_left: bytes_left })
        } else {
            // Hash the complete piece contained in the current mmap
            let hash = ShaHash::from_bytes(&region[next_region_offset..end_region_offset]);
            
            // Send the completed piece hash back to the master
            send.send(MasterMessage::AcceptPiece(next_piece_index.get(), hash)).unwrap();
        }
        
        // Grab a new piece index
        next_piece_index = index_queue.get();
        // Calculate the new piece index offset
        next_region_offset = calculate_piece_offset(region_bytes_processed, next_piece_index.get(), piece_length);
    }
    
    // Put the index back into the queue for processing later
    index_queue.put_back(next_piece_index);
    
    None
}

/// Processes the partial piece from one region extending onto the next region.
///
/// Returns the state back if it needs to be processed with the next region, or None it is has finished.
fn process_overlapping_region(mut prev_state: PartialHashState, curr_region: &[u8], send: &Sender<MasterMessage>)
    -> Option<PartialHashState> {
    let bytes_left = prev_state.bytes_left;
    
    if bytes_left > curr_region.len() {
        // We are going to have to process this partial piece again after this
        prev_state.bytes_left -= curr_region.len();
        prev_state.builder = prev_state.builder.add_bytes(curr_region);
        
        Some(prev_state)
    } else {
        // We are not going to have to process this partial peice again after this
        prev_state.bytes_left = 0;
        let hash = prev_state.builder.add_bytes(&curr_region[0..bytes_left]).build();
        
        // Send the processed piece
        send.send(MasterMessage::AcceptPiece(prev_state.piece_index, hash)).unwrap();
        
        None
    }
}

/// Calculates the offset into the current mmap that the given piece index resides at.
fn calculate_piece_offset(region_bytes_processed: usize, piece_index: usize, piece_length: usize) -> usize {
    // Offset in relation to all previously processed mmaps
    let global_offset = piece_index * piece_length;
    
    // Local offset into the current (or future) mmap
    global_offset - region_bytes_processed
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc};
    use std::sync::mpsc::{self};

    use bip_util::sha::{ShaHash};
    use rand::{self, Rng};
    
    use builder::queue::{IndexQueue};
    use builder::worker::{self, MasterMessage};
    
    // Keep these numbers fairly small to avoid lengthy tests
    const DEFAULT_PIECE_LENGTH: usize = 1024;
    const DEFAULT_NUM_PIECES:   usize = 500;
    const DEFAULT_NUM_REGIONS:  usize = 4;
    
    /// Generates a buffer of random bytes with the specified size.
    fn generate_random_bytes(size: usize) -> Vec<u8> {
        let mut buffer = vec![0u8; size];
        let mut rng = rand::thread_rng();
        
        rng.fill_bytes(&mut buffer);
        
        buffer
    }
    
    #[test]
    fn positive_piece_index_zero() {
        let bytes_processed = 0;
        let piece_index = 0;
    
        let expected_offset = 0;
        assert_eq!(super::calculate_piece_offset(bytes_processed, piece_index, DEFAULT_PIECE_LENGTH),
            expected_offset);
    }
    
    #[test]
    fn positive_piece_index_one() {
        let bytes_processed = 0;
        let piece_index = 1;
        
        let expected_offset = DEFAULT_PIECE_LENGTH;
        assert_eq!(super::calculate_piece_offset(bytes_processed, piece_index, DEFAULT_PIECE_LENGTH),
            expected_offset);
    }
    
    #[test]
    fn positive_piece_index_larget() {
        let bytes_processed = 0;
        let piece_index = 2034;
        
        let expected_offset = DEFAULT_PIECE_LENGTH * 2034;
        assert_eq!(super::calculate_piece_offset(bytes_processed, piece_index, DEFAULT_PIECE_LENGTH),
            expected_offset);
    }
    
    #[test]
    fn positive_process_piece_length_divisible_region() {
        let region_length = DEFAULT_PIECE_LENGTH * DEFAULT_NUM_PIECES;
        let region = generate_random_bytes(region_length);
        
        // Make sure our region has the correct length
        assert_eq!(region.len(), region_length);
        
        // Let the routine process our region
        let (send, recv) = mpsc::channel();
        let index_queue = Arc::new(IndexQueue::new());
        let opt_partial_state = worker::process_region(&region, &index_queue, 0, DEFAULT_PIECE_LENGTH, &send);
        
        // Should not have any partial piece left
        assert!(opt_partial_state.is_none());
        
        // Don't simply iterate otherwise we would block (if we are failing fail NOW)
        let mut supposed_pieces = Vec::with_capacity(DEFAULT_NUM_PIECES);
        for _ in 0..DEFAULT_NUM_PIECES {
            if let Ok(MasterMessage::AcceptPiece(index, piece)) = recv.try_recv() {
                supposed_pieces.push((index, piece));
            } else {
                panic!("Receiver failed to receive a piece...")
            }
        }
        assert!(recv.try_recv().is_err());
        // Since we did not process the pieces in multiple threads, we know the pieces are already in order
        
        // Calculate the pieces ourselves to check against our previous result
        let mut calculated_pieces = Vec::with_capacity(DEFAULT_NUM_PIECES);
        for (index, chunk) in region.chunks(DEFAULT_PIECE_LENGTH).enumerate() {
            let piece = ShaHash::from_bytes(chunk);
            
            calculated_pieces.push((index, piece));
        }
        
        // Assert that our calculated pieces are equal to the supposed pieces
        assert_eq!(calculated_pieces, supposed_pieces);
    }
    
    #[test]
    fn positive_process_piece_length_divisible_regions() {
        let region_length = DEFAULT_PIECE_LENGTH * DEFAULT_NUM_PIECES;
        
        let mut regions = Vec::with_capacity(DEFAULT_NUM_REGIONS);
        for _ in 0..DEFAULT_NUM_REGIONS {
            regions.push(generate_random_bytes(region_length));
        }
        
        // Make sure our region has the designated number of bytes
        let region_size_sum = regions.iter().map(|r| r.len()).fold(0, |sum, len| sum + len);
        assert_eq!(region_size_sum, DEFAULT_NUM_REGIONS * region_length);
        
        // Compute the supposed pieces for all of our regions
        let (send, recv) = mpsc::channel();
        let index_queue = Arc::new(IndexQueue::new());
        let mut bytes_processed = 0;
        for region in regions.iter() {
            let opt_partial_state = worker::process_region(region, &index_queue, bytes_processed,
                DEFAULT_PIECE_LENGTH, &send);
            
            // All regions are a multiple of the piece length, no partial states!
            assert!(opt_partial_state.is_none());
            
            bytes_processed += region.len();
        }
        
        let expected_pieces = DEFAULT_NUM_PIECES * DEFAULT_NUM_REGIONS;
        
        // Gather the expected amount of pieces
        let mut supposed_pieces = Vec::with_capacity(expected_pieces);
        for _ in 0..expected_pieces {
            if let Ok(MasterMessage::AcceptPiece(index, piece)) = recv.try_recv() {
                supposed_pieces.push((index, piece));
            } else {
                panic!("Receiver failed to receive a piece...")
            }
        }
        assert!(recv.try_recv().is_err());
        
        // Process the regions ourselves
        let mut calculated_pieces = Vec::with_capacity(expected_pieces);
        for region in regions.iter() {
            for chunk in region.chunks(DEFAULT_PIECE_LENGTH) {
                let piece_index = calculated_pieces.len();
                let piece = ShaHash::from_bytes(chunk);
                
                calculated_pieces.push((piece_index, piece));
            }
        }
        
        // Assert that our calculated pieces are equal to the supposed pieces
        assert_eq!(calculated_pieces, supposed_pieces);
    }
    
    #[test]
    fn positive_process_piece_length_undivisible_region() {
        let region_length = (DEFAULT_PIECE_LENGTH * DEFAULT_NUM_PIECES) - 1;
        let region = generate_random_bytes(region_length);
        
        // Make sure our region has the correct length
        assert_eq!(region.len(), region_length);
        
        let (send, recv) = mpsc::channel();
        let index_queue = Arc::new(IndexQueue::new());
        let opt_partial_state = worker::process_region(&region, &index_queue, 0, DEFAULT_PIECE_LENGTH, &send);
        
        // Perform some checks on the partial piece
        let partial_state = opt_partial_state.unwrap();
        assert_eq!(partial_state.bytes_left, 1);
        
        let mut supposed_pieces = Vec::with_capacity(DEFAULT_NUM_PIECES - 1);
        for _ in 0..(DEFAULT_NUM_PIECES - 1) {
            if let Ok(MasterMessage::AcceptPiece(index, piece)) = recv.try_recv() {
                supposed_pieces.push((index, piece));
            } else {
                panic!("Receiver failed to receive a piece...")
            }
        }
        assert!(recv.try_recv().is_err());
        
        // Push the last partial piece manually
        supposed_pieces.push((partial_state.piece_index, partial_state.builder.build()));
        
        let mut calculated_pieces = Vec::with_capacity(DEFAULT_NUM_PIECES);
        for (index, chunk) in region.chunks(DEFAULT_PIECE_LENGTH).enumerate() {
            let piece = ShaHash::from_bytes(chunk);
            
            calculated_pieces.push((index, piece));
        }
        
        assert_eq!(calculated_pieces, supposed_pieces);
    }
    
    #[test]
    fn positive_process_piece_length_undivisible_regions() {
        // Two regions that are smaller than the piece length, one that is bigger
        let region_one_length = DEFAULT_PIECE_LENGTH - 1;
        let region_two_length = DEFAULT_PIECE_LENGTH / 2;
        let region_three_length = DEFAULT_NUM_PIECES * DEFAULT_PIECE_LENGTH;
        
        // Make our region contiguous so it is easier for us to validate the pieces
        let contiguous_region_length = region_one_length + region_two_length + region_three_length;
        let contiguous_region = generate_random_bytes(contiguous_region_length);
        
        assert_eq!(contiguous_region.len(), contiguous_region_length);
        
        let (send, recv) = mpsc::channel();
        let index_queue = Arc::new(IndexQueue::new());
        let mut bytes_processed = 0;
        
        let region_one = &contiguous_region[0..region_one_length];
        let region_two = &contiguous_region[region_one_length..(region_one_length + region_two_length)];
        let region_three = &contiguous_region[(region_one_length + region_two_length)..];
        
        // Process region one
        let partial_state_one = worker::process_region(region_one, &index_queue, bytes_processed, DEFAULT_PIECE_LENGTH,
            &send).unwrap();
        // The first region overlaps into the second, we need to re-process it, but the remainder bytes should
        // ALL be contained in the second region, so we dont need to process it with the third region.
        assert!(worker::process_overlapping_region(partial_state_one, region_two, &send).is_none());
        
        bytes_processed += region_one.len();
        
        // Process region two
        let partial_state_two = worker::process_region(region_two, &index_queue, bytes_processed, DEFAULT_PIECE_LENGTH,
            &send).unwrap();
        // Second region overlaps into the third region
        assert!(worker::process_overlapping_region(partial_state_two, region_three, &send).is_none());
        
        bytes_processed += region_two.len();
        
        // Process region three
        let partial_state_three = worker::process_region(region_three, &index_queue, bytes_processed, DEFAULT_PIECE_LENGTH,
            &send).unwrap();
        // Since the second region overlapped into the third and took a number of bytes not equal to the piece length,
        // we dont have bytes that are divisible by the piece length which means we will also have a partial state.
        // However, since this is the last partial state, we just need to manually get the hash of the last partial piece.
        
        let expected_pieces = contiguous_region_length / DEFAULT_PIECE_LENGTH;
        
        let mut supposed_pieces = Vec::with_capacity(expected_pieces + 1);
        for _ in 0..expected_pieces {
            if let Ok(MasterMessage::AcceptPiece(index, piece)) = recv.try_recv() {
                supposed_pieces.push((index, piece));
            } else {
                panic!("Receiver failed to receive a piece...")
            }
        }
        assert!(recv.try_recv().is_err());
        
        // Push the last partial piece manually
        supposed_pieces.push((partial_state_three.piece_index, partial_state_three.builder.build()));
        
        let mut calculated_pieces = Vec::with_capacity(expected_pieces + 1);
        for (index, chunk) in contiguous_region.chunks(DEFAULT_PIECE_LENGTH).enumerate() {
            let piece = ShaHash::from_bytes(chunk);
            
            calculated_pieces.push((index, piece));
        }
        
        assert_eq!(calculated_pieces, supposed_pieces);
    }
}