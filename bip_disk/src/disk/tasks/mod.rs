use disk::fs::FileSystem;
use disk::{IDiskMessage, ODiskMessage};
use disk::tasks::helpers::piece_checker::{PieceChecker, PieceCheckerState, PieceState};
use disk::tasks::helpers::piece_accessor::PieceAccessor;
use disk::tasks::context::DiskManagerContext;
use memory::block::{Block, BlockMut};
use error::{TorrentResult, BlockResult, BlockError, BlockErrorKind, TorrentError, TorrentErrorKind};

use bip_metainfo::Metainfo;
use bip_util::bt::InfoHash;
use futures::sink::Wait;
use futures::sync::mpsc::Sender;
use futures_cpupool::CpuPool;

pub mod context;
mod helpers;

pub fn execute_on_pool<F>(msg: IDiskMessage, pool: &CpuPool, context: DiskManagerContext<F>)
    where F: FileSystem + Send + Sync + 'static {
    pool.spawn_fn(move || {
        let mut blocking_sender = context.blocking_sender();

        let out_msg = match msg {
            IDiskMessage::AddTorrent(metainfo) => {
                let info_hash = metainfo.info().info_hash();
                
                match execute_add_torrent(metainfo, &context, &mut blocking_sender) {
                    Ok(_)    => ODiskMessage::TorrentAdded(info_hash),
                    Err(err) => ODiskMessage::TorrentError(info_hash, err)
                }
            },
            IDiskMessage::RemoveTorrent(hash) => {
                match execute_remove_torrent(hash, &context) {
                    Ok(_)    => ODiskMessage::TorrentRemoved(hash),
                    Err(err) => ODiskMessage::TorrentError(hash, err)
                }
            },
            IDiskMessage::SyncTorrent(hash) => {
                match execute_sync_torrent(hash, &context) {
                    Ok(_)    => ODiskMessage::TorrentSynced(hash),
                    Err(err) => ODiskMessage::TorrentError(hash, err)
                }
            },
            IDiskMessage::LoadBlock(mut block) => {
                match execute_load_block(&mut block, &context) {
                    Ok(_)    => ODiskMessage::BlockLoaded(block),
                    Err(err) => ODiskMessage::LoadBlockError(block, err)
                }
            },
            IDiskMessage::ProcessBlock(mut block) => {
                match execute_process_block(&mut block, &context, &mut blocking_sender) {
                    Ok(_)    => ODiskMessage::BlockProcessed(block),
                    Err(err) => ODiskMessage::ProcessBlockError(block, err)
                }
            }
        };

        blocking_sender.send(out_msg)
            .expect("bip_disk: Failed To Send Out Message In execute_on_pool");
        blocking_sender.flush()
            .expect("bip_disk: Failed to Flush Out Messages In execute_on_pool");
        
        Ok::<(),()>(())
    }).forget()
}

fn execute_add_torrent<F>(file: Metainfo, context: &DiskManagerContext<F>, blocking_sender: &mut Wait<Sender<ODiskMessage>>) -> TorrentResult<()>
    where F: FileSystem {
    let info_hash = file.info().info_hash();
    let mut init_state = try!(PieceChecker::init_state(context.filesystem(), file.info()));

    // In case we are resuming a download, we need to send the diff for the newly added torrent
    send_piece_diff(&mut init_state, info_hash, blocking_sender, true);
    
    if context.insert_torrent(file, init_state) {
        Ok(())
    } else {
        Err(TorrentError::from_kind(TorrentErrorKind::ExistingInfoHash{ hash: info_hash }))
    }
}

fn execute_remove_torrent<F>(hash: InfoHash, context: &DiskManagerContext<F>) -> TorrentResult<()>
    where F: FileSystem {
    if context.remove_torrent(hash) {
        Ok(())
    } else {
        Err(TorrentError::from_kind(TorrentErrorKind::InfoHashNotFound{ hash: hash }))
    }
}

fn execute_sync_torrent<F>(hash: InfoHash, context: &DiskManagerContext<F>) -> TorrentResult<()>
    where F: FileSystem {
    let filesystem = context.filesystem();

    let mut sync_result = Ok(());
    let found_hash = context.update_torrent(hash, |metainfo_file, _| {
        let opt_parent_dir = metainfo_file.info().directory();

        for file in metainfo_file.info().files() {
            let path = helpers::build_path(opt_parent_dir, file);

            sync_result = filesystem.sync_file(path);
        }
    });

    if found_hash {
        Ok(try!(sync_result))
    } else {
        Err(TorrentError::from_kind(TorrentErrorKind::InfoHashNotFound{ hash: hash }))
    }
}

fn execute_load_block<F>(block: &mut BlockMut, context: &DiskManagerContext<F>) -> BlockResult<()>
    where F: FileSystem {
    let metadata = block.metadata();
    let info_hash = metadata.info_hash();

    let mut access_result = Ok(());
    let found_hash = context.update_torrent(info_hash, |metainfo_file, _| {
        let piece_accessor = PieceAccessor::new(context.filesystem(), metainfo_file.info());

        // Read The Piece In From The Filesystem
        access_result = piece_accessor.read_piece(&mut *block, &metadata)
    });

    if found_hash {
        Ok(try!(access_result))
    } else {
        Err(BlockError::from_kind(BlockErrorKind::InfoHashNotFound{ hash: info_hash }))
    }
}

fn execute_process_block<F>(block: &mut Block, context: &DiskManagerContext<F>, blocking_sender: &mut Wait<Sender<ODiskMessage>>) -> BlockResult<()>
    where F: FileSystem {
    let metadata = block.metadata();
    let info_hash = metadata.info_hash();

    let mut block_result = Ok(());
    let found_hash = context.update_torrent(info_hash, |metainfo_file, mut checker_state| {
        info!("Processsing Block, Acquired Torrent Lock For {:?}", metainfo_file.info().info_hash());

        let piece_accessor = PieceAccessor::new(context.filesystem(), metainfo_file.info());

        // Write Out Piece Out To The Filesystem And Recalculate The Diff
        block_result = piece_accessor.write_piece(&block, &metadata)
            .and_then(|_| {
                checker_state.add_pending_block(metadata);
                
                PieceChecker::with_state(context.filesystem(), metainfo_file.info(), &mut checker_state)
                    .calculate_diff()
            });

        send_piece_diff(checker_state, metainfo_file.info().info_hash(), blocking_sender, false);

        info!("Processsing Block, Released Torrent Lock For {:?}", metainfo_file.info().info_hash());
    });

    if found_hash {
        Ok(try!(block_result))
    } else {
        Err(BlockError::from_kind(BlockErrorKind::InfoHashNotFound{ hash: info_hash }))
    }
}

fn send_piece_diff(checker_state: &mut PieceCheckerState, hash: InfoHash, blocking_sender: &mut Wait<Sender<ODiskMessage>>, ignore_bad: bool) {
    checker_state.run_with_diff(|piece_state| {
        let opt_out_msg = match (piece_state, ignore_bad) {
            (&PieceState::Good(index), _)    => Some(ODiskMessage::FoundGoodPiece(hash, index)),
            (&PieceState::Bad(index), false) => Some(ODiskMessage::FoundBadPiece(hash, index)),
            (&PieceState::Bad(_), true)      => None
        };

        if let Some(out_msg) = opt_out_msg {
            blocking_sender.send(out_msg)
                .expect("bip_disk: Failed To Send Piece State Message");
            blocking_sender.flush()
            .expect("bip_disk: Failed To Flush Piece State Message");
        }
    })
}