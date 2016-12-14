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
use disk::fs::{FileSystem};
use token::Token;
use message::standard::PieceMessage;

pub mod shared;

const NUM_DISK_WORKERS: usize = 4;
const NUM_BLOCK_WORKERS: usize = 4;

pub enum DiskMessage {
    AddTorrent(Token, MetainfoFile, PathBuf),
    RemoveTorrent(Token, InfoHash),
    LoadBlock(Token, Token, InfoHash, PieceMessage),
    ProcessBlock(Token, Token)
}

pub enum SyncBlockMessage {
    ReserveBlock(Token, Token, InfoHash, PieceMessage)
}

pub enum AsyncBlockMessage {
    ReclaimBlock(Token, Token)
}

pub struct ReserveBlockClientMetadata {
    hash:    InfoHash,
    message: PieceMessage
}

// ----------------------------------------------------------------------------//

pub fn create_workers<F>(fs: F, clients: Arc<Clients<ReserveBlockClientMetadata>>, blocks: Arc<Blocks>)
    -> (Sender<DiskMessage>, Sender<SyncBlockMessage>, Sender<AsyncBlockMessage>)
    where F: FileSystem + Send + Sync + 'static {
    unimplemented!()
}