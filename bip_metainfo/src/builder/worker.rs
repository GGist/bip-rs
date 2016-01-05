use std::fs::{File};
use std::io::{self, Read};
use std::sync::{Arc};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread::{self};

use bip_util::sha::{ShaHash};
use crossbeam::sync::{MsQueue};
use walkdir::{DirEntry};

use builder::buffer::{PieceBuffers, PieceBuffer};
use error::{ParseError, ParseResult};

/// Trait used to access data that is stored at some location.
pub trait DataEntry: Send {
    type Data: Read;
    
    fn access(&self) -> io::Result<<Self as DataEntry>::Data>;
}

impl DataEntry for DirEntry {
    type Data = File;
    
    fn access(&self) -> io::Result<File> {
        File::open(self.path())
    }
}

/// Messages sent to the master hasher.
pub enum MasterMessage<T> where T: DataEntry {
    /// Originates from the client!!!
    ///
    /// Process the next file in order.
    IncludeEntry(T),
    /// Originates from the client!!!
    ///
    /// This means there are no more entries.
    ClientFinished,
    /// Originates from a worker!!!
    ///
    /// Accepts the piece hash with the given piece index.
    AcceptPiece(usize, ShaHash),
    /// Originates from a worker!!!
    ///
    /// One of our workers has finished and shut down.
    WorkerFinished
}

/// Message received from the master hasher.
pub enum ClientMessage {
    /// Hashing process completed.
    Completed(Vec<(usize, ShaHash)>),
    /// Hashing process errored.
    Errored(ParseError)
}

/// Message sent to a worker hasher.
#[derive(PartialEq, Eq)]
enum WorkerMessage {
    /// Start processing the given memory region.
    ///
    /// Size of the region as well as the region are given.
    HashPiece(usize, PieceBuffer),
    /// Worker should either exit it's thread or hash
    /// the last (irregular) piece that it was holding on to.
    Finish
}

// Create master worker, read in chunks of file (only whole pieces, unless we receive a finish), send to a sync sender, 

/// Starts a number of hasher workers which will generate the hash pieces for the files we send to it.
pub fn start_hasher_workers<T>(piece_length: usize, num_workers: usize) -> (Sender<MasterMessage<T>>, Receiver<ClientMessage>)
    where T: DataEntry + 'static {
    // Create channels to communicate with the master and client
    let (master_send, master_recv) = mpsc::channel();
    let (client_send, client_recv) = mpsc::channel();
    
    // Create queue to push work to and pull work from
    let work_queue = Arc::new(MsQueue::new());
    
    // Create buffer allocator to reuse pre allocated buffer
    let piece_buffers = Arc::new(PieceBuffers::new(piece_length, num_workers));
    
    // Create n worker threads that pull work from the queue
    for _ in 0..num_workers {
        let share_master_send = master_send.clone();
        let share_work_queue = work_queue.clone();
        let share_piece_buffers = piece_buffers.clone();
        
        thread::spawn(move || {
            start_hash_worker(share_master_send, share_work_queue, share_piece_buffers);
        });
    }
    
    // Create the master worker to coordinate between the workers and the client
    thread::spawn(move || {
        match start_hash_master(num_workers, master_recv, work_queue, piece_buffers) {
            Ok(pieces) => client_send.send(ClientMessage::Completed(pieces)).unwrap(),
            Err(error) => client_send.send(ClientMessage::Errored(error)).unwrap()
        }
    });
    
    (master_send, client_recv)
}

//----------------------------------------------------------------------------//

/// Start a master hasher which will take care of chunking sequential/overlapping pieces from the data given to it and giving updates to the hasher workers.
fn start_hash_master<T>(num_workers: usize, recv: Receiver<MasterMessage<T>>, work: Arc<MsQueue<WorkerMessage>>,
    buffers: Arc<PieceBuffers>) -> ParseResult<Vec<(usize, ShaHash)>> where T: DataEntry {
    let mut pieces = Vec::new();
    let mut piece_index = 0;
    
    let mut cont_piece_buffer = buffers.checkout();
    // Loop through all messages until client sends a finished event
    for msg in recv.iter() {
        match msg {
            MasterMessage::WorkerFinished            => panic!("bip_metainfo: Worker finished unexpectedly..."),
            MasterMessage::AcceptPiece(index, piece) => pieces.push((index, piece)),
            MasterMessage::ClientFinished            => { break; },
            MasterMessage::IncludeEntry(entry)       => { 
                cont_piece_buffer = try!(distribute_data_entry(entry, &mut piece_index, &*work, cont_piece_buffer, &*buffers));
            }
        }
    }
    
    // Push last possibly partial piece
    if !cont_piece_buffer.is_empty() {
        work.push(WorkerMessage::HashPiece(piece_index, cont_piece_buffer));
    }
    
    // No more entries, tell workers to shut down
    for _ in 0..num_workers {
        work.push(WorkerMessage::Finish);
    }
    
    // Wait for all of the workers to finish up the last pieces
    let mut workers_finished = 0;
    while workers_finished < num_workers {
        match recv.recv() {
            Ok(MasterMessage::AcceptPiece(index, piece)) => pieces.push((index, piece)),
            Ok(MasterMessage::WorkerFinished)            => workers_finished += 1,
            Ok(MasterMessage::ClientFinished)            => panic!("bip_metainfo: Client sent duplicates finishes..."),
            Ok(MasterMessage::IncludeEntry(..))          => panic!("bip_metainfo: Client sent an entry after finishing..."),
            Err(_)                                       => panic!("bip_metainfo: Master failed to verify all workers shutdown...")
        }
    }
    
    // Sort our list to make sure the pieces are in order before we send them off
    pieces.sort_by(|one, two| {
        one.0.cmp(&two.0)
    });
    
    Ok(pieces)
}

/// Process the given data entry, pushing whole pieces onto the worker queue.
///
/// Returns the partial (or empty) piece buffer that should be added to if more entries are seen.
fn distribute_data_entry<T>(entry: T, piece_index: &mut usize, work: &MsQueue<WorkerMessage>, cont_buffer: PieceBuffer,
    buffers: &PieceBuffers) -> io::Result<PieceBuffer> where T: DataEntry {
    let mut readable_data = try!(entry.access());
    let mut piece_buffer = cont_buffer;
    
    let mut eof = false;
    while !eof {
        eof = try!(piece_buffer.read_bytes(|buffer_slice| {
            readable_data.read(buffer_slice)
        }));
        
        if piece_buffer.is_whole() {
            work.push(WorkerMessage::HashPiece(*piece_index, piece_buffer));
            
            *piece_index += 1;
            piece_buffer = buffers.checkout();
        }
    }
    
    Ok(piece_buffer)
}

//----------------------------------------------------------------------------//

/// Starts a hasher worker which will hash all of the buffers it receives.
fn start_hash_worker<T>(send: Sender<MasterMessage<T>>, work: Arc<MsQueue<WorkerMessage>>, buffers: Arc<PieceBuffers>)
    where T: DataEntry {
    let mut work_to_do = true;
    
    while work_to_do {
        let work_item = work.pop();
        
        match work_item {
            WorkerMessage::Finish                   => { work_to_do = false; },
            WorkerMessage::HashPiece(index, buffer) => {
                let hash = ShaHash::from_bytes(buffer.as_slice());
                
                send.send(MasterMessage::AcceptPiece(index, hash)).unwrap();
                buffers.checkin(buffer);
            }
        }
    }
    
    send.send(MasterMessage::WorkerFinished).unwrap();
}

#[cfg(test)]
mod tests {
    use std::thread::{self};
    use std::time::{Duration};
    use std::io::{self, Cursor};

    use bip_util::sha::{ShaHash};
    use rand::{self, Rng};
    
    use builder::worker::{self, MasterMessage, DataEntry, ClientMessage};
    
    // Keep these numbers fairly small to avoid lengthy tests
    const DEFAULT_PIECE_LENGTH: usize = 1024;
    const DEFAULT_NUM_PIECES:   usize = 300;
    
    // Mock object for providing direct access to bytes via DataEntry.
    #[derive(Clone)]
    struct MockDataEntry {
        buffer: Vec<u8>
    }
    
    impl MockDataEntry {
        /// Creates a new MockDataEntry with the given number of bytes initialized with random data.
        fn as_random(num_bytes: usize) -> MockDataEntry {
            let mut buffer = vec![0u8; num_bytes];
            let mut rng = rand::thread_rng();
            
            rng.fill_bytes(&mut buffer);
            
            MockDataEntry{ buffer: buffer }
        }
        
        /// Access the internal bytes as a slice.
        fn as_slice(&self) -> &[u8] {
            &self.buffer
        }
    }
    
    // Data has to be 'static since we are sending it. Also, Vec doesnt implement read...
    impl DataEntry for MockDataEntry {
        type Data = Cursor<Vec<u8>>;
        
        fn access(&self) -> io::Result<Cursor<Vec<u8>>> {
            Ok(Cursor::new(self.buffer.clone()))
        }
    }
    
    /// Test helper which will validate that the pieces calculated from the given entries are correctly calculated
    /// by the hash workers.
    fn validate_entries_pieces(data_entries: Vec<MockDataEntry>, piece_length: usize, num_threads: usize) {
        // Start up the hasher workers
        let (send, recv) = worker::start_hasher_workers(piece_length, num_threads);
        
        // Calculate a contiguous form of the given entries for easy chunking
        let contiguous_entry = data_entries.iter().map(|entry| entry.as_slice()).fold(vec![], |mut acc, next| {
            acc.extend_from_slice(next);
            
            acc
        });
        let computed_pieces = contiguous_entry.chunks(piece_length).enumerate().map(|(index, chunk)| {
            (index, ShaHash::from_bytes(chunk))
        }).collect::<Vec<(usize, ShaHash)>>();
        
        // Send each of the data entries to the hasher
        for data_entry in data_entries {
            send.send(MasterMessage::IncludeEntry(data_entry)).unwrap();
        }
        send.send(MasterMessage::ClientFinished).unwrap();
        
        // Allow the hasher to finish up it's worker threads
        thread::sleep(Duration::from_millis(100));
        
        // Receive the calculated pieces from the hasher
        let received_pieces = match recv.recv().unwrap() {
            ClientMessage::Completed(pieces) => pieces,
            ClientMessage::Errored(error)    => panic!("Piece Hasher Errored Out: {:?}", error)
        };
        
        // Make sure our computes piece values match up with those computed by the hasher
        assert_eq!(received_pieces, computed_pieces);
    }
    
    #[test]
    fn positive_piece_length_divisible_region_single_thread() {
        let region_length = DEFAULT_PIECE_LENGTH * DEFAULT_NUM_PIECES;
        let data_entry = vec![MockDataEntry::as_random(region_length)];
    
        validate_entries_pieces(data_entry, DEFAULT_PIECE_LENGTH, 1);
    }
    
    #[test]
    fn positive_piece_length_divisible_region_multiple_threads() {
        let region_length = DEFAULT_PIECE_LENGTH * DEFAULT_NUM_PIECES;
        let data_entry = vec![MockDataEntry::as_random(region_length)];
    
        validate_entries_pieces(data_entry, DEFAULT_PIECE_LENGTH, 4);
    }
    
    #[test]
    fn positive_piece_length_undivisible_region_single_thread() {
        let region_length = DEFAULT_PIECE_LENGTH * DEFAULT_NUM_PIECES + 1;
        let data_entry = vec![MockDataEntry::as_random(region_length)];
        
        validate_entries_pieces(data_entry, DEFAULT_PIECE_LENGTH, 1);
    }
    
    #[test]
    fn positive_piece_length_undivisible_region_multiple_threads() {
        let region_length = DEFAULT_PIECE_LENGTH * DEFAULT_NUM_PIECES + 1;
        let data_entry = vec![MockDataEntry::as_random(region_length)];
        
        validate_entries_pieces(data_entry, DEFAULT_PIECE_LENGTH, 4);
    }
    
    #[test]
    fn positive_piece_length_divisible_regions_single_thread() {
        let region_lengths = [
            DEFAULT_PIECE_LENGTH * DEFAULT_NUM_PIECES,
            DEFAULT_PIECE_LENGTH * 1,
            DEFAULT_PIECE_LENGTH * 50
        ];
        let data_entries = region_lengths.iter().map(|&length| 
            MockDataEntry::as_random(length)
        ).collect();
        
        validate_entries_pieces(data_entries, DEFAULT_PIECE_LENGTH, 1);
    }
    
    #[test]
    fn positive_piece_length_divisible_regions_multiple_threads() {
        let region_lengths = [
            DEFAULT_PIECE_LENGTH * DEFAULT_NUM_PIECES,
            DEFAULT_PIECE_LENGTH * 1,
            DEFAULT_PIECE_LENGTH * 50
        ];
        let data_entries = region_lengths.iter().map(|&length| 
            MockDataEntry::as_random(length)
        ).collect();
        
        validate_entries_pieces(data_entries, DEFAULT_PIECE_LENGTH, 4);
    }
    
    #[test]
    fn positive_piece_length_undivisible_regions_single_thread() {
        let region_lengths = [
            DEFAULT_PIECE_LENGTH / 2 * DEFAULT_NUM_PIECES,
            DEFAULT_PIECE_LENGTH / 4 * DEFAULT_NUM_PIECES,
            DEFAULT_PIECE_LENGTH * 1,
            (DEFAULT_PIECE_LENGTH * 2 - 1) * 2
        ];
        let data_entries = region_lengths.iter().map(|&length|
            MockDataEntry::as_random(length)
        ).collect();
        
        validate_entries_pieces(data_entries, DEFAULT_PIECE_LENGTH, 1);
    }
    
    #[test]
    fn positive_piece_length_undivisible_regions_multiple_threads() {
        let region_lengths = [
            DEFAULT_PIECE_LENGTH / 2 * DEFAULT_NUM_PIECES,
            DEFAULT_PIECE_LENGTH / 4 * DEFAULT_NUM_PIECES,
            DEFAULT_PIECE_LENGTH * 1,
            (DEFAULT_PIECE_LENGTH * 2 - 1) * 2
        ];
        let data_entries = region_lengths.iter().map(|&length|
            MockDataEntry::as_random(length)
        ).collect();
        
        validate_entries_pieces(data_entries, DEFAULT_PIECE_LENGTH, 4);
    }
}