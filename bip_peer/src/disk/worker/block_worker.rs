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
use disk::worker::{ReserveBlockClientMetadata, SyncBlockMessage, AsyncBlockMessage};
use token::Token;
use message::standard::PieceMessage;

/// Spawn a synchronous block worker thread.
///
/// Returns a channel to send work to the block worker thread.
pub fn spawn_sync_block_worker(clients: Arc<Clients<ReserveBlockClientMetadata>>, blocks: Arc<Blocks>) -> Sender<SyncBlockMessage> {
    let (send, recv) = chan::async();

    thread::spawn(move || {
        for msg in recv {
            match msg {
                SyncBlockMessage::ReserveBlock(callback_namespace, namespace, request, hash, piece_msg) => {
                    clients.associate_metadata(namespace, request, ReserveBlockClientMetadata::new(hash, piece_msg));
                    blocks.allocate_block(namespace, request, piece_msg.block_length());

                    clients.message_client(callback_namespace, ODiskMessage::BlockReserved(namespace, request));
                }
            }
        }
    });

    send
}

/// Spawn an asynchronous block worker thread.
///
/// Returns a channel to send work to the block worker thread.
pub fn spawn_async_block_worker(blocks: Arc<Blocks>) -> Sender<AsyncBlockMessage> {
    let (send, recv) = chan::async();

    thread::spawn(move || {
        for msg in recv {
            match msg {
                AsyncBlockMessage::ReclaimBlock(namespace, request) => {
                    blocks.reclaim_block(namespace, request);
                }
            }
        }
    });

    send
}