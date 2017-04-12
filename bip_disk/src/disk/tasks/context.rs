use std::sync::{Arc, RwLock, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;
use std::mem;

use disk::fs::FileSystem;
use disk::ODiskMessage;
use disk::tasks::helpers::piece_checker::PieceCheckerState;
use error::{TorrentResult, TorrentError, TorrentErrorKind};

use bip_metainfo::MetainfoFile;
use bip_util::bt::InfoHash;
use futures::sync::mpsc::Sender;
use futures::sink::Sink;
use futures::Future;
use futures_cpupool::CpuPool;
use tokio_core::reactor::Handle;

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

    pub fn filesystem(&self) -> &F {
        &self.fs
    }

    pub fn can_submit_work(&self) -> bool {
        let prev_value = self.cur_pending.fetch_add(1, Ordering::SeqCst);

        if prev_value < self.max_pending {
            true
        } else {
            self.cur_pending.fetch_sub(1, Ordering::SeqCst);

            false
        }
    }

    pub fn complete_work(self, opt_out_msg: Option<ODiskMessage>) {
        if let Some(out_msg) = opt_out_msg {
            // TODO: Should we check the result of wait here??
            self.out.send(out_msg).wait();
        }

        self.cur_pending.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn insert_torrent(&self, file: MetainfoFile, state: PieceCheckerState) -> TorrentResult<()> {
        let mut write_torrents = self.torrents.write()
            .expect("bip_disk: DiskManagerContext::insert_torrents Failed To Write Torrent");

        let hash = file.info_hash();
        if write_torrents.contains_key(&hash) {
            Err(TorrentError::from_kind(TorrentErrorKind::ExistingInfoHash{ hash: hash }))
        } else {
            write_torrents.insert(hash, Mutex::new(MetainfoState::new(file, state)));

            Ok(())
        }
    }

    pub fn update_torrent<C>(&self, hash: InfoHash, call: C) -> TorrentResult<()>
        where C: FnOnce(&MetainfoFile, PieceCheckerState) -> PieceCheckerState {
        let read_torrents = self.torrents.read()
            .expect("bip_disk: DiskManagerContext::update_torrent Failed To Read Torrent");

        match read_torrents.get(&hash) {
            Some(state) => {
                let mut lock_state = state.lock()
                    .expect("bip_disk: DiskManagerContext::update_torrent Failed To Lock State");
                
                let old_state = mem::replace(&mut lock_state.state, PieceCheckerState::new(0, 0));

                let new_state = call(&lock_state.file, old_state);
                lock_state.state = new_state;

                Ok(())
            },
            None => Err(TorrentError::from_kind(TorrentErrorKind::InfoHashNotFound{ hash: hash }))
        }
    }

    pub fn remove_torrent(&self, hash: InfoHash) -> TorrentResult<()> {
        let mut write_torrents = self.torrents.write()
            .expect("bip_disk: DiskManagerContext::remove_torrent Failed To Write Torrent");

        write_torrents.remove(&hash)
            .map(|_| ())
            .ok_or_else(|| {
                TorrentError::from_kind(TorrentErrorKind::InfoHashNotFound{ hash: hash })
            })
    }
}

impl<F> Clone for DiskManagerContext<F> {
    fn clone(&self) -> DiskManagerContext<F> {
        DiskManagerContext{ torrents: self.torrents.clone(), out: self.out.clone(),
                            fs: self.fs.clone(), cur_pending: self.cur_pending.clone(),
                            max_pending: self.max_pending }
    }
}