use error::{TorrentError, BlockError};
use memory::block::{Block};

use bip_metainfo::MetainfoFile;
use bip_util::bt::{InfoHash};

pub mod builder;
pub mod manager;
pub mod fs;
mod tasks;

//----------------------------------------------------------------------------//

/// Messages that can be sent to the `DiskManager`.
#[derive(Debug)]
pub enum IDiskMessage {
    AddTorrent(MetainfoFile),
    RemoveTorrent(InfoHash),
    LoadBlock(Block),
    ProcessBlock(Block)
}

/// Messages that can be received from the `DiskManager`.
#[derive(Debug)]
pub enum ODiskMessage {
    TorrentAdded(InfoHash),
    TorrentRemoved(InfoHash),
    FoundGoodPiece(InfoHash, u64),
    FoundBadPiece(InfoHash, u64),
    BlockLoaded(Block),
    BlockProcessed(Block),
    TorrentError(InfoHash, TorrentError),
    BlockError(Block, BlockError)
}