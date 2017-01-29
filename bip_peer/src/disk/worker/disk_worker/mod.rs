use std::sync::{Arc};
use std::thread;

use chan::{self, Sender};

use disk::worker::shared::blocks::Blocks;
use disk::worker::shared::clients::Clients;
use disk::worker::{ReserveBlockClientMetadata, SyncBlockMessage, AsyncBlockMessage, DiskMessage};
use disk::worker::disk_worker::context::DiskWorkerContext;
use disk::fs::{FileSystem};
use disk;
use token::{Token};

mod context;
mod piece_checker;
mod piece_accessor;

pub fn spawn_disk_worker<F>(fs: F, clients: Arc<Clients<ReserveBlockClientMetadata>>, blocks: Arc<Blocks>, sync_worker: Sender<SyncBlockMessage>,
    async_worker: Sender<AsyncBlockMessage>, disk_worker_namespace: Token) -> Sender<DiskMessage> where F: FileSystem + Send + Sync + 'static {
    let (send, recv) = chan::async();

    let disk_context = Arc::new(DiskWorkerContext::new(send.clone(), fs, clients, blocks, sync_worker, async_worker, disk_worker_namespace));

    for _ in 0..disk::DISK_MANAGER_WORKER_THREADS {
        let clone_disk_context = disk_context.clone();
        let clone_recv = recv.clone();
        
        thread::spawn(move || {
            for msg in clone_recv {
                match msg {
                    DiskMessage::AddTorrent(namespace, metainfo)                => clone_disk_context.add_torrent(namespace, metainfo),
                    DiskMessage::RemoveTorrent(namespace, hash)                 => clone_disk_context.remove_torrent(namespace, hash),
                    DiskMessage::LoadBlock(namespace, request, hash, piece_msg) => clone_disk_context.load_block(namespace, request, hash, piece_msg),
                    DiskMessage::ProcessBlock(namespace, request)               => clone_disk_context.process_block(namespace, request),
                    DiskMessage::BlockReserved(namespace, request)              => clone_disk_context.block_reserved(namespace, request),
                    DiskMessage::RequestError(request_error)                    => clone_disk_context.request_error(request_error)
                }
            }
        });
    }

    send
}