use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use disk::fs::FileSystem;
use disk::{IDiskMessage, ODiskMessage};
use disk::tasks::helpers::piece_checker::PieceChecker;
use disk::tasks::context::DiskManagerContext;
use token::Token;
use memory::block::Block;
use error::{TorrentResult, BlockResult};

use bip_metainfo::MetainfoFile;
use bip_util::bt::InfoHash;
use futures::sync::mpsc::Sender;
use futures_cpupool::CpuPool;
use tokio_core::reactor::Handle;

pub mod context;
mod helpers;

pub fn execute_on_pool<F>(msg: IDiskMessage, pool: &CpuPool, context: DiskManagerContext<F>)
    where F: FileSystem + Send + Sync + 'static {
    pool.spawn_fn(move || {
        let opt_out_msg = match msg {
            IDiskMessage::AddTorrent(metainfo) => {
                let info_hash = metainfo.info_hash();

                execute_add_torrent(metainfo, &context)
                    .err()
                    .map(|torr_err| ODiskMessage::TorrentError(info_hash, torr_err))
            },
            IDiskMessage::RemoveTorrent(hash) => {
                execute_remove_torrent(hash, &context)
                    .err()
                    .map(|torr_err| ODiskMessage::TorrentError(hash, torr_err))
            },
            IDiskMessage::LoadBlock(namespace, request, mut block) => {
                execute_load_block(namespace, request, &mut block, &context)
                    .err()
                    .map(|block_err| ODiskMessage::BlockError(block, block_err))
            },
            IDiskMessage::ProcessBlock(mut block) => {
                execute_process_block(&mut block, &context)
                    .err()
                    .map(|block_err| ODiskMessage::BlockError(block, block_err))
            }
        };

        context.complete_work(opt_out_msg);

        Ok::<(),()>(())
    }).forget()
}

fn execute_add_torrent<F>(file: MetainfoFile, context: &DiskManagerContext<F>) -> TorrentResult<()>
    where F: FileSystem {
    let init_state = {
        try!(PieceChecker::new(context.filesystem(), file.info())
            .and_then(|checker| checker.calculate_diff()))
    };

    try!(context.insert_torrent(file, init_state));
    Ok(())
}

fn execute_remove_torrent<F>(hash: InfoHash, context: &DiskManagerContext<F>) -> TorrentResult<()>
    where F: FileSystem {
    context.remove_torrent(hash)
}

fn execute_load_block<F>(namespace: Token, request: Token, block: &mut Block, context: &DiskManagerContext<F>) -> BlockResult<()>
    where F: FileSystem {
    unimplemented!()
}

fn execute_process_block<F>(block: &mut Block, context: &DiskManagerContext<F>) -> BlockResult<()>
    where F: FileSystem {
    unimplemented!()
}