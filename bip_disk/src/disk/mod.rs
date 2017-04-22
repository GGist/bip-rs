use std::default::Default;

use error::{TorrentError, BlockError};
use memory::block::{Block, BlockMetadata};
use token::Token;

use bip_metainfo::MetainfoFile;
use bip_util::bt::{self, InfoHash};

pub mod builder;
pub mod manager;
pub mod fs;
mod tasks;

//----------------------------------------------------------------------------//

/// Messages that can be sent to the `DiskManager`.
pub enum IDiskMessage {
    AddTorrent(MetainfoFile),
    RemoveTorrent(InfoHash),
    LoadBlock(Token, Token, Block),
    ProcessBlock(Token, Token, Block)
}

/// Messages that can be received from the `DiskManager`.
pub enum ODiskMessage {
    TorrentAdded(InfoHash),
    TorrentRemoved(InfoHash),
    FoundGoodPiece(InfoHash, u64),
    FoundBadPiece(InfoHash, u64),
    BlockLoaded(Token, Token, Block),
    BlockProcessed(Token, Token, Block),
    TorrentError(InfoHash, TorrentError),
    BlockError(Block, BlockError)
}