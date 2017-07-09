use std::sync::{Arc, RwLock, Mutex};
use std::collections::HashMap;

use disk::ODiskMessage;
use disk::tasks::helpers::piece_checker::PieceCheckerState;

use bip_metainfo::MetainfoFile;
use bip_util::bt::InfoHash;
use futures::sync::mpsc::Sender;
use futures::sink::Sink;
use futures::sink::Wait;

pub struct DiskManagerContext<F> {
    torrents:    Arc<RwLock<HashMap<InfoHash, Mutex<MetainfoState>>>>,
    out:         Sender<ODiskMessage>,
    fs:          Arc<F>
}

pub struct MetainfoState {
    file:  MetainfoFile,
    state: PieceCheckerState
}

impl MetainfoState {
    pub fn new(file: MetainfoFile, state: PieceCheckerState) -> MetainfoState {
        MetainfoState{ file: file, state: state }
    }
}

impl<F> DiskManagerContext<F> {
    pub fn new(out: Sender<ODiskMessage>, fs: F) -> DiskManagerContext<F> {
        DiskManagerContext{ torrents: Arc::new(RwLock::new(HashMap::new())), out: out, fs: Arc::new(fs) }
    }

    pub fn blocking_sender(&self) -> Wait<Sender<ODiskMessage>> {
        self.out.clone().wait()
    }

    pub fn filesystem(&self) -> &F {
        &self.fs
    }

    pub fn insert_torrent(&self, file: MetainfoFile, state: PieceCheckerState) -> bool {
        let mut write_torrents = self.torrents.write()
            .expect("bip_disk: DiskManagerContext::insert_torrents Failed To Write Torrent");

        let hash = file.info_hash();
        let hash_not_exists = !write_torrents.contains_key(&hash);

        if hash_not_exists {
            write_torrents.insert(hash, Mutex::new(MetainfoState::new(file, state)));
        }

        hash_not_exists
    }

    pub fn update_torrent<C>(&self, hash: InfoHash, call: C) -> bool
        where C: FnOnce(&MetainfoFile, &mut PieceCheckerState) {
        let read_torrents = self.torrents.read()
            .expect("bip_disk: DiskManagerContext::update_torrent Failed To Read Torrent");

        match read_torrents.get(&hash) {
            Some(state) => {
                let mut lock_state = state.lock()
                    .expect("bip_disk: DiskManagerContext::update_torrent Failed To Lock State");
                let mut deref_state = &mut *lock_state;

                call(&deref_state.file, &mut deref_state.state);

                true
            },
            None => false
        }
    }

    pub fn remove_torrent(&self, hash: InfoHash) -> bool {
        let mut write_torrents = self.torrents.write()
            .expect("bip_disk: DiskManagerContext::remove_torrent Failed To Write Torrent");

        write_torrents.remove(&hash)
            .map(|_| true)
            .unwrap_or(false)
    }
}

impl<F> Clone for DiskManagerContext<F> {
    fn clone(&self) -> DiskManagerContext<F> {
        DiskManagerContext{ torrents: self.torrents.clone(), out: self.out.clone(), fs: self.fs.clone() }
    }
}