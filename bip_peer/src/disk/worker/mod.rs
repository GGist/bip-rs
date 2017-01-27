use std::io;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::path::PathBuf;

use bip_metainfo::MetainfoFile;
use bip_util::bt::InfoHash;
use bip_util::send::TrySender;
use chan::{self, Sender, Receiver};

use disk::worker::shared::blocks::Blocks;
use disk::worker::shared::clients::Clients;
use disk::{IDiskMessage, ODiskMessage};
use disk::error::RequestError;
use disk::fs::{FileSystem};
use token::Token;
use message::standard::PieceMessage;

pub mod shared;
mod block_worker;
mod disk_worker;

const NUM_DISK_WORKERS: usize = 4;

pub enum DiskMessage {
    AddTorrent(Token, MetainfoFile),
    RemoveTorrent(Token, InfoHash),
    LoadBlock(Token, Token, InfoHash, PieceMessage),
    ProcessBlock(Token, Token),
    /// INTERNAL USE ONLY
    BlockReserved(Token, Token),
    RequestError(RequestError)
}

pub enum SyncBlockMessage {
    ReserveBlock(Token, Token, Token, InfoHash, PieceMessage)
}

pub enum AsyncBlockMessage {
    ReclaimBlock(Token, Token)
}

pub struct ReserveBlockClientMetadata {
    pub hash:    InfoHash,
    pub message: PieceMessage

}

impl ReserveBlockClientMetadata {
    pub fn new(hash: InfoHash, message: PieceMessage) -> ReserveBlockClientMetadata {
        ReserveBlockClientMetadata { hash: hash, message: message }
    }
}

// ----------------------------------------------------------------------------//

pub fn create_workers<F>(fs: F, clients: Arc<Clients<ReserveBlockClientMetadata>>, blocks: Arc<Blocks>,
    disk_worker_namespace: Token) -> (Sender<DiskMessage>, Sender<SyncBlockMessage>, Sender<AsyncBlockMessage>)
    where F: FileSystem + Send + Sync + 'static {
    let sync_worker = block_worker::spawn_sync_block_worker(clients.clone(), blocks.clone());
    let async_worker = block_worker::spawn_async_block_worker(blocks.clone());
    let disk_worker = disk_worker::spawn_disk_worker(fs, clients, blocks, sync_worker.clone(), disk_worker_namespace);

    (disk_worker, sync_worker, async_worker)
}