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
use disk::worker::{self, ReserveBlockClientMetadata, SyncBlockMessage, AsyncBlockMessage, DiskMessage};
use disk::worker::disk_worker::context::DiskWorkerContext;
use disk::fs::{FileSystem};
use token::{Token, TokenGenerator};
use message::standard::PieceMessage;

mod context;

pub fn spawn_disk_worker<F>(fs: F, clients: Arc<Clients<ReserveBlockClientMetadata>>, blocks: Arc<Blocks>,
    block_worker: Sender<SyncBlockMessage>, disk_worker_namespace: Token) -> Sender<DiskMessage>
    where F: FileSystem + Send + Sync + 'static {
    let (send, recv) = chan::async();

    let disk_context = Arc::new(DiskWorkerContext::new(send.clone(), fs, clients, blocks, block_worker, disk_worker_namespace));

    for _ in 0..worker::NUM_DISK_WORKERS {
        let clone_disk_context = disk_context.clone();
        let clone_recv = recv.clone();

        thread::spawn(move || {
            for msg in clone_recv {
                match msg {
                    DiskMessage::AddTorrent(namespace, metainfo, base_dir)      => clone_disk_context.add_torrent(namespace, metainfo, base_dir),
                    DiskMessage::RemoveTorrent(namespace, hash)                 => clone_disk_context.remove_torrent(namespace, hash),
                    DiskMessage::LoadBlock(namespace, request, hash, piece_msg) => clone_disk_context.load_block(namespace, request, hash, piece_msg),
                    DiskMessage::ProcessBlock(namespace, request)               => clone_disk_context.process_block(namespace, request),
                    DiskMessage::BlockReserved(request)                         => clone_disk_context.block_reserved(request),
                    DiskMessage::RequestError(request_error)                    => clone_disk_context.request_error(request_error)
                }
            }
        });
    }

    send
}