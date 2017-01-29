use std::collections::HashMap;
use std::collections::hash_map::{Entry};
use std::sync::{Arc, Mutex, RwLock};
use std::mem;
use std::io::Write;

use bip_metainfo::MetainfoFile;
use bip_util::bt::InfoHash;
use bip_util::send::TrySender;
use bip_util::contiguous::ContiguousBuffer;
use chan::{Sender};

use disk::worker::shared::blocks::Blocks;
use disk::worker::shared::clients::Clients;
use disk::worker::disk_worker::piece_accessor::PieceAccessor;
use disk::{ODiskMessage};
use disk::error::{RequestError, TorrentError, TorrentResult, TorrentErrorKind};
use disk::fs::{FileSystem};
use disk::worker::disk_worker::piece_checker::{PieceChecker, PieceState, PieceCheckerState};
use disk::worker::{ReserveBlockClientMetadata, SyncBlockMessage, DiskMessage, AsyncBlockMessage};
use token::{Token};
use message::standard::PieceMessage;

pub struct DiskWorkerContext<F> {
    fs:              F,
    torrents:        RwLock<HashMap<InfoHash, Mutex<TorrentEntry>>>,
    clients:         Arc<Clients<ReserveBlockClientMetadata>>,
    blocks:          Arc<Blocks>,
    sync_worker:     Sender<SyncBlockMessage>,
    async_worker:    Sender<AsyncBlockMessage>,
    namespace_token: Token
}

struct TorrentEntry {
    metainfo:         MetainfoFile,
    checker_state:    PieceCheckerState,
    client_namespace: Token
}

impl TorrentEntry {
    fn new(metainfo: MetainfoFile, checker_state: PieceCheckerState, client_namespace: Token) -> TorrentEntry {
        TorrentEntry{
            metainfo: metainfo,
            checker_state: checker_state,
            client_namespace: client_namespace
        }
    }
}

impl<F> DiskWorkerContext<F> where F: FileSystem {
    pub fn new(send: Sender<DiskMessage>, fs: F, clients: Arc<Clients<ReserveBlockClientMetadata>>, blocks: Arc<Blocks>,
        sync_worker: Sender<SyncBlockMessage>, async_worker: Sender<AsyncBlockMessage>, disk_worker_namespace: Token)
        -> DiskWorkerContext<F> {
        // Add ourselves to the clients structure, this allows us to request blocks to be reserved
        // from the block worker when we, for example, need to load a block from disk.
        clients.add_client(disk_worker_namespace, Box::new(DiskSender(send)));

        DiskWorkerContext {
            fs: fs,
            torrents: RwLock::new(HashMap::new()),
            clients: clients,
            blocks: blocks,
            sync_worker: sync_worker,
            async_worker: async_worker,
            namespace_token: disk_worker_namespace
        }
    }

    pub fn add_torrent(&self, namespace: Token, metainfo: MetainfoFile) {
        let hash = metainfo.info_hash();

        let res_checker_state = PieceChecker::new(&self.fs, metainfo.info())
            .and_then(|checker| checker.calculate_diff())
            .and_then(|checker_state| {
                let torrent_entry = TorrentEntry::new(metainfo, checker_state, namespace);

                self.insert_torrent_entry(torrent_entry)
            });

        match res_checker_state {
            Ok(_) => {
                self.clients.message_client(namespace, ODiskMessage::TorrentAdded(hash));

                self.access_torrent_entry_mut(&hash, |mut entry| {
                    entry.checker_state.run_with_diff(|piece_state| {
                        // Since this is the initial diff, don't let clients know of bad pieces since these were reloaded from disk
                        match piece_state {
                            &PieceState::Good(index) => self.clients.message_client(namespace, ODiskMessage::FoundGoodPiece(hash, index)),
                            &PieceState::Bad(_)      => ()
                        }
                    });
                });
            },
            Err(torrent_error) => self.clients.message_client(namespace, ODiskMessage::TorrentError(torrent_error))
        }
    }

    pub fn remove_torrent(&self, namespace: Token, hash: InfoHash) {
        match self.remove_torrent_entry(hash) {
            Ok(_)              => (),
            Err(torrent_error) => self.clients.message_client(namespace, ODiskMessage::TorrentError(torrent_error))
        }
    }

    pub fn load_block(&self, namespace: Token, request: Token, hash: InfoHash, piece_msg: PieceMessage) {
        self.sync_worker.send(SyncBlockMessage::ReserveBlock(self.namespace_token, namespace, request, hash, piece_msg));
    }

    pub fn process_block(&self, namespace: Token, request: Token) {
        let metadata = self.clients.remove_metadata(namespace, request);
        let (hash, piece_message) = (metadata.hash, metadata.message);

        // Well, the API I spent so long on, Blocks, is useless since we eventually have to pass
        // a mutable reference to a byte array (which most OS's require, barring using a smallish
        // buffer to transfer data from disk). Big TODO here...
        let mut buffer = vec![0u8; piece_message.block_length()];
        (*self.blocks).access_block(namespace, request, |buffers| {
            let mut bytes_read = 0;

            buffers.read(|slice| {
                (&mut buffer[bytes_read..]).write(slice)
                    .expect("bip_peer: Failed To Copy Block Into Buffer");
                bytes_read += slice.len();
            });
        });

        // TODO: Handle fs failures
        self.access_torrent_entry_mut(&hash, |mut entry| {
            let piece_accessor = PieceAccessor::new(&self.fs, entry.metainfo.info());

            piece_accessor.write_piece(&buffer[..], &piece_message)
                .expect("bip_peer: Failed To Write Piece To Disk");

            // Add piece message to piece checker state
            entry.checker_state.add_pending_block(piece_message);

            // Its more efficient to swap here, otherwise, we would have to take a write
            // lock on the outer HashMap to remove, then again to add this back.
            let checker_state = mem::replace(&mut entry.checker_state, PieceCheckerState::new(0, 0));
            let piece_checker = PieceChecker::with_state(&self.fs, entry.metainfo.info(), checker_state);
            
            // TODO: Handle failure here
            let mut new_checker_state = piece_checker.calculate_diff()
                .expect("bip_peer: Failed To Access Disk For Hashing");
            
            new_checker_state.run_with_diff(|piece_state| {
                // Since this is the initial diff, don't let clients know of bad pieces since these were reloaded from disk
                match piece_state {
                    &PieceState::Good(index) => self.clients.message_client(entry.client_namespace, ODiskMessage::FoundGoodPiece(hash, index)),
                    &PieceState::Bad(index)  => self.clients.message_client(entry.client_namespace, ODiskMessage::FoundBadPiece(hash, index))
                }
            });

            entry.checker_state = new_checker_state;
        });

        // Reclaim the block
        self.async_worker.send(AsyncBlockMessage::ReclaimBlock(namespace, request));
    }

    pub fn block_reserved(&self, namespace: Token, request: Token) {
        let metadata = self.clients.remove_metadata(namespace, request);
        let (hash, piece_message) = (metadata.hash, metadata.message);
        
        // Well, the API I spent so long on, Blocks, is useless since we eventually have to pass
        // a mutable reference to a byte array (which most OS's require, barring using a smallish
        // buffer to transfer data from disk). Big TODO here...
        let mut buffer = vec![0u8; piece_message.block_length()];
        (*self.blocks).access_block(namespace, request, |mut buffers| {
                buffers.write(&buffer[..]);
        });

        // TODO: Handle fs failures
        self.access_torrent_entry(&hash, |entry| {
            let piece_accessor = PieceAccessor::new(&self.fs, entry.metainfo.info());

            piece_accessor.read_piece(&mut buffer[..], &piece_message)
                .expect("bip_peer: Failed To Read Piece From Disk");
        });

        self.clients.message_client(namespace, ODiskMessage::BlockLoaded(namespace, request));
    }

    pub fn request_error(&self, _request_error: RequestError) {
        // TODO: Heh, we should figure out what to do here
        unimplemented!()
    }

    fn access_torrent_entry_mut<C>(&self, hash: &InfoHash, mut callback: C)
        where C: FnMut(&mut TorrentEntry) {
        let read_torrents = self.torrents.read()
            .expect("bip_peer: Failed To Get Write Lock On Torrents Map");
        let mut write_torrent = read_torrents.get(hash)
            .expect("bip_peer: Failed To Lookup Torrent Entry In Map")
            .lock()
            .expect("bip_peer: Failed To Lock Torrent Entry In Map");

        callback(&mut write_torrent);
    }

    fn access_torrent_entry<C>(&self, hash: &InfoHash, mut callback: C)
        where C: FnMut(&TorrentEntry) {
        let read_torrents = self.torrents.read()
            .expect("bip_peer: Failed To Get Write Lock On Torrents Map");
        let mut write_torrent = read_torrents.get(hash)
            .expect("bip_peer: Failed To Lookup Torrent Entry In Map")
            .lock()
            .expect("bip_peer: Failed To Lock Torrent Entry In Map");

        callback(&mut write_torrent);
    }

    fn insert_torrent_entry(&self, entry: TorrentEntry) -> TorrentResult<()> {
        let mut write_torrents = self.torrents.write()
            .expect("bip_peer: Failed To Get Write Lock On Torrents Map");
        let hash = entry.metainfo.info_hash();

        match write_torrents.entry(hash) {
            Entry::Vacant(vac) => {
                vac.insert(Mutex::new(entry));
                Ok(())
            },
            Entry::Occupied(_) => Err(TorrentError::from_kind(TorrentErrorKind::ExistingInfoHash{ hash: hash }))
        }
    }

    fn remove_torrent_entry(&self, hash: InfoHash) -> TorrentResult<()> {
        let mut write_torrents = self.torrents.write()
            .expect("bip_peer: Failed To Get Write Lock On Torrents Map");

        write_torrents.remove(&hash)
            .map(|_| ())
            .ok_or(TorrentError::from_kind(TorrentErrorKind::InfoHashNotFound{ hash: hash }))
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
            ODiskMessage::BlockReserved(namespace, request) => self.0.send(DiskMessage::BlockReserved(namespace, request)),
            ODiskMessage::RequestError(request)             => self.0.send(DiskMessage::RequestError(request)),
            msg => panic!("DiskSender Was Given An Invalid Message From The Block Worker: {:?}", msg)
        }

        None
    }
}