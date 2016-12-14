#![allow(unused)]

use std::sync::{Arc, Mutex};
use std::sync::mpsc::SyncSender;
use std::path::PathBuf;
use std::io::{self, Cursor, Write};
use std::cell::RefCell;

use bip_metainfo::MetainfoFile;
use bip_util::bt::{InfoHash};
use bip_util::send::TrySender;
use bip_util::contiguous::ContiguousBuffer;
use chan::{Sender};

use disk::worker::{DiskMessage, SyncBlockMessage, AsyncBlockMessage, ReserveBlockClientMetadata};
use disk::worker::shared::clients::Clients;
use disk::worker::shared::blocks::Blocks;
use disk::error::{RequestError, TorrentError};
use registration::LayerRegistration;
use token::{Token, TokenGenerator};
use message::standard::PieceMessage;

mod fs;
mod error;
mod worker;

pub use disk::fs::{FileSystem, OSFileSystem, MemFileSystem};

const DISK_MANAGER_WORKER_THREADS: usize = 16;

// Maximum as well as the default block size for our requests.
const DEFAULT_BLOCK_SIZE: usize = 16 * 1024;

// Maximum allowed block size for peers requesting from us.
const MAX_ALLOWED_BLOCK_SIZE: usize = 32 * 1024;

// Because a single piece may come from multiple peers, we need to track
// how many bytes a single peer contibruted to both a good piece, as well
// as a bad piece. If a peer is seen as having less than 75% of total
// bytes sent to us as good, they are considered bad. At the same time,
// we only enforce this for peers who have sent us more than a given
// number of bytes.
const MALICIOUS_PEER_MIN_GOOD_HASH_RATE: f32 = 0.75;
// Want this to be larger than a single piece since that is when we will be able
// to detect if a piece is good or not. Hopefully a good peer isnt always sending
// in the same piece as a malicious peer (should probably make some guarantee here).
// For a 16KB block size and 16MB piece size, this would be 25 pieces.
const MALICIOUS_PEER_MIN_TOTAL_BYTES: usize = DEFAULT_BLOCK_SIZE * 1024 * 25;

/// Message that can be sent to the disk manager.
#[derive(Debug)]
pub enum IDiskMessage {
    /// Add the given torrent at the specified path to the DiskManager.
    ///
    /// The sender will also be signed up to receive `ODiskMessage::FoundGoodPiece`,
    /// `ODiskMessage::FoundBadpiece`, and `ODiskMessage::TorrentError` messages.
    AddTorrent(MetainfoFile, PathBuf),
    /// Remove the torrent from the disk manager.
    ///
    /// This does NOT delete anything from disk.
    ///
    /// The sender MAY receive a single `ODiskMessage::TorrentError` message.
    RemoveTorrent(InfoHash),
    /// Load the block from the InfoHash into memory.
    LoadBlock(Token, InfoHash, PieceMessage),
    /// Reclaim and mark the block as unused.
    ReclaimBlock(Token),
    /// Reserve space for the block belonging to the InfoHash.
    ReserveBlock(Token, InfoHash, PieceMessage),
    /// Reclaimn the block and process it.
    ProcessBlock(Token)
}

/// Message that can be received from the disk manager.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ODiskMessage {
    /// DiskManager has assembled and verified a good the given piece at the index.
    FoundGoodPiece(InfoHash, usize),
    /// DiskManager has assembled and verified a bad piece at the index.
    FoundBadPiece(InfoHash, usize),
    /// Block for the given token has been loaded.
    BlockLoaded(Token),
    /// Block for the given token has been reserved.
    BlockReserved(Token),
    /// Errors that can occur from a request associated with a CompoundToken.
    RequestError(RequestError),
    /// Errors that can occur from a request associated with an InfoHash.
    TorrentError(TorrentError)
}

/// Helper for calculating the needed worst case queue size for a client receiver.
pub fn client_recv_queue_length(total_pieces: usize) -> usize {
    // Assumes that a FoundBadPiece cannot be followed by a FoundGoodPiece both referring
    // to the same piece UNLESS the selection layer has observed the FoundBadPiece message.
    // So we need room for all our Found[Good|Bad]Piece messages as well as a possible
    // TorrentError message.
    total_pieces + 1
}

/// Helper for calculating the needed worst case queue size for a peer (protocol) receiver.
pub fn peer_recv_queue_length(max_outgoing: usize) -> usize {
    // If max outgoing is 5, and selection layer sends 5 piece messages to peer
    // that will be 5 WaitLoad messages. Then the protocol layer transfers to
    // a read state. It then receives a piece message which corresponds to a
    // WaitReserve message. At this point, the selection layer experiecnes
    // back pressure from the peer protocol layer, and the peer protocol layer
    // experiences back pressure from the disk manager. So the total number
    // of messages that the peer should be ready to accept is 6 or 5 + 1.
    max_outgoing + 1
}

// ----------------------------------------------------------------------------//

/// Central place for clients to register themselves to access a DiskManager.
pub struct DiskManagerRegistration {
    namespace_gen:      TokenGenerator,
    clients:            Arc<Clients<ReserveBlockClientMetadata>>,
    blocks:             Arc<Blocks>,
    disk_sender:        Sender<DiskMessage>,
    sync_block_sender:  Sender<SyncBlockMessage>,
    async_block_sender: Sender<AsyncBlockMessage>,
}

impl DiskManagerRegistration {
    /// Create a new DiskManagerRegistration using the given FileSystem.
    pub fn with_fs<F>(fs: F) -> DiskManagerRegistration
        where F: FileSystem + Send + Sync + 'static {
        // Create the shared data structures.
        let clients = Arc::new(Clients::new());
        let blocks = Arc::new(Blocks::new(DEFAULT_BLOCK_SIZE));

        // Spin up new worker threads for allocating blocks and writing them to disk.
        let (disk_sender, sb_sender, ab_sender) = worker::create_workers(fs, clients.clone(), blocks.clone());

        DiskManagerRegistration {
            namespace_gen: TokenGenerator::new(),
            clients: clients,
            blocks: blocks,
            disk_sender: disk_sender,
            sync_block_sender: sb_sender,
            async_block_sender: ab_sender
        }
    }
}

impl LayerRegistration<ODiskMessage, IDiskMessage> for DiskManagerRegistration {
    type SS2 = DiskManager;

    fn register(&mut self, send: Box<TrySender<ODiskMessage>>) -> DiskManager {
        let registration_token = self.namespace_gen.generate();

        // The token we used to resgister will be our token "namespace", all messages we
        // send will have an associated id, these ids will be namespace by this token.
        DiskManager::new(registration_token, self.clients.clone(), self.blocks.clone(),
                         self.disk_sender.clone(), self.sync_block_sender.clone(),
                         self.async_block_sender.clone(), send)
    }
}

// ----------------------------------------------------------------------------//

/// Trait to allow additional methods on the SS2 for DiskManagerRegistration.
pub trait DiskManagerAccess {
    /// Access a reserved block and write the given bytes.
    fn write_block(&self, token: Token, read_bytes: &[u8]);

    /// Access a loaded block and read the given bytes.
    fn read_block(&self, token: Token, write_bytes: &mut Write);

    /// Generate a new request token.
    fn new_request_token(&mut self) -> Token;
}

/// DiskManager that allows clients to send messages to workers in charge
/// of allocating blocks of memory, as well as writing blocks to disk.
pub struct DiskManager {
    namespace:          Token,
    request_gen:        TokenGenerator,
    clients:            Arc<Clients<ReserveBlockClientMetadata>>,
    blocks:             Arc<Blocks>,
    disk_sender:        Sender<DiskMessage>,
    sync_block_sender:  Sender<SyncBlockMessage>,
    async_block_sender: Sender<AsyncBlockMessage>,
}

impl DiskManager {
    /// Create a new DiskManager.
    pub fn new(namespace: Token, clients: Arc<Clients<ReserveBlockClientMetadata>>, blocks: Arc<Blocks>,
               disk_sender: Sender<DiskMessage>, sb_sender: Sender<SyncBlockMessage>, ab_sender: Sender<AsyncBlockMessage>,
               client_sender: Box<TrySender<ODiskMessage>>) -> DiskManager {
        clients.add_client(namespace, client_sender);
        blocks.register_namespace(namespace);

        DiskManager {
            namespace: namespace,
            request_gen: TokenGenerator::new(),
            clients: clients,
            blocks: blocks,
            disk_sender: disk_sender,
            sync_block_sender: sb_sender,
            async_block_sender: ab_sender
        }
    }
}

impl DiskManagerAccess for DiskManager {
    fn write_block(&self, token: Token, read_bytes: &[u8]) {
        self.blocks.access_block(self.namespace, token, |mut block| {
            block.write(read_bytes);
        });
    }

    fn read_block(&self, token: Token, write_bytes: &mut Write) {
        self.blocks.access_block(self.namespace, token, |mut block| {
            block.read(|bytes| {
                match write_bytes.write(bytes) {
                    Ok(num_written) if num_written == bytes.len() => (),
                    _ => panic!("bip_peer: DiskManagerAccess Failed To Write All Bytes")
                }
            });
        });
    }

    fn new_request_token(&mut self) -> Token {
        self.request_gen.generate()
    }
}

impl TrySender<IDiskMessage> for DiskManager {
    fn try_send(&self, data: IDiskMessage) -> Option<IDiskMessage> {
        unimplemented!();
    }
}

impl Drop for DiskManager {
    fn drop(&mut self) {
        self.clients.remove_client(self.namespace);
        self.blocks.unregister_namespace(self.namespace);
    }
}