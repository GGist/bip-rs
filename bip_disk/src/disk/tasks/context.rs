use std::sync::{Arc, RwLock, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
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
    fs:          Arc<F>,
    cur_pending: Arc<AtomicUsize>,
    max_pending: usize
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
    pub fn new(out: Sender<ODiskMessage>, fs: F, max_pending: usize) -> DiskManagerContext<F> {
        DiskManagerContext{ torrents: Arc::new(RwLock::new(HashMap::new())), out: out, fs: Arc::new(fs),
                            cur_pending: Arc::new(AtomicUsize::new(0)), max_pending: max_pending }
    }

    pub fn blocking_sender(&self) -> Wait<Sender<ODiskMessage>> {
        self.out.clone().wait()
    }

    pub fn filesystem(&self) -> &F {
        &self.fs
    }

    pub fn try_submit_work(&self) -> bool {
        let prev_value = self.cur_pending.fetch_add(1, Ordering::SeqCst);

        if prev_value < self.max_pending {
            info!("Submitted Work, Previous Pending Was {} New Pending Is {} Of Max {}", prev_value, prev_value + 1, self.max_pending);

            true
        } else {
            self.cur_pending.fetch_sub(1, Ordering::SeqCst);

            info!("Failed To Submit Work, Pending Is {} Of Max {}", prev_value, self.max_pending);

            false
        }
    }

    pub fn can_submit_work(&self) -> bool {
        self.cur_pending.load(Ordering::SeqCst) < self.max_pending
    }

    pub fn complete_work(&self) {
        let prev_pending = self.cur_pending.fetch_sub(1, Ordering::SeqCst);

        info!("Completed Work, Previous Pending Was {} New Pending Is {}", prev_pending, prev_pending - 1);
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
        DiskManagerContext{ torrents: self.torrents.clone(), out: self.out.clone(),
                            fs: self.fs.clone(), cur_pending: self.cur_pending.clone(),
                            max_pending: self.max_pending }
    }
}