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
use disk::worker::{ReserveBlockClientMetadata, SyncBlockMessage, AsyncBlockMessage, DiskMessage};
use token::Token;
use message::standard::PieceMessage;

pub struct DiskWorkerContext<F> {
    fs:              F,
    torrents:        HashMap<InfoHash, MetainfoFile>,
    clients:         Arc<Clients<ReserveBlockClientMetadata>>,
    blocks:          Arc<Blocks>,
    block_worker:    Sender<SyncBlockMessage>,
    namespace_token: Token
}

impl<F> DiskWorkerContext<F> where F: FileSystem {
    pub fn new(send: Sender<DiskMessage>, fs: F, clients: Arc<Clients<ReserveBlockClientMetadata>>, blocks: Arc<Blocks>,
        block_worker: Sender<SyncBlockMessage>, disk_worker_namespace: Token) -> DiskWorkerContext<F> {
        // Add ourselves to the clients structure, this allows us to request blocks to be reserved
        // from the block worker when we, for example, need to load a block from disk.
        clients.add_client(disk_worker_namespace, Box::new(DiskSender(send)));

        DiskWorkerContext {
            fs: fs,
            torrents: HashMap::new(),
            clients: clients,
            blocks: blocks,
            block_worker: block_worker,
            namespace_token: disk_worker_namespace
        }
    }

    pub fn add_torrent(&self, namespace: Token, metainfo: MetainfoFile, base_dir: PathBuf) {
        unimplemented!()
    }

    pub fn remove_torrent(&self, namespace: Token, hash: InfoHash) {
        unimplemented!()
    }

    pub fn load_block(&self, namespace: Token, request: Token, hash: InfoHash, piece_msg: PieceMessage) {
        unimplemented!()
    }

    pub fn process_block(&self, namespace: Token, request: Token) {
        unimplemented!()
    }

    pub fn block_reserved(&self, request: Token) {
        unimplemented!()
    }

    pub fn request_error(&self, request_error: RequestError) {
        unimplemented!()
    }
}

impl<F> Drop for DiskWorkerContext<F> {
    fn drop(&mut self) {
        self.clients.remove_client(self.namespace_token);
    }
}

// ----------------------------------------------------------------------------//

pub struct DiskSender(Sender<DiskMessage>);

impl TrySender<ODiskMessage> for DiskSender {
    fn try_send(&self, data: ODiskMessage) -> Option<ODiskMessage> {
        // Disk workers will send messages to the block workers, but the block workers only send
        // ODiskMessage types back to clients. Our disk workers are only interested in a subset of
        // those responses, any other messages indicate a programming error.
        match data {
            ODiskMessage::BlockReserved(token) => self.0.send(DiskMessage::BlockReserved(token)),
            ODiskMessage::RequestError(token) => self.0.send(DiskMessage::RequestError(token)),
            msg => panic!("DiskSender Was Given An Invalid Message From The Block Worker: {:?}", msg)
        }

        None
    }
}