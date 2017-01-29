use std::sync::{Arc};
use std::thread;

use chan::{self, Sender};

use disk::worker::shared::blocks::Blocks;
use disk::worker::shared::clients::Clients;
use disk::{ODiskMessage};
use disk::worker::{ReserveBlockClientMetadata, SyncBlockMessage, AsyncBlockMessage};

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