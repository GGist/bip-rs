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
    /// Message to add a torrent to the disk manager.
    AddTorrent(MetainfoFile),
    /// Message to remove a torrent from the disk manager.
    ///
    /// Note, this will NOT remove any data from the `FileSystem`,
    /// and as an added convenience, this message will also trigger
    /// a `IDiskMessage::SyncTorrent` message.
    RemoveTorrent(InfoHash),
    /// Message to tell the `FileSystem` to sync the torrent.
    ///
    /// This message will trigger a call to `FileSystem::sync` for every
    /// file in the torrent, so the semantics will differ depending on the
    /// `FileSystem` in use.
    ///
    /// In general, if a torrent has finished downloading, but will be kept
    /// in the `DiskManager` to, for example, seed the torrent, then this
    /// message should be sent, otherwise, `IDiskMessage::RemoveTorrent` is
    /// sufficient.
    SyncTorrent(InfoHash),
    /// Message to load the given block in to memory.
    LoadBlock(Block),
    /// Message to process the given block and persist it.
    ProcessBlock(Block)
}

/// Messages that can be received from the `DiskManager`.
#[derive(Debug)]
pub enum ODiskMessage {
    /// Message indicating that the torrent has been added.
    ///
    /// Any good pieces already existing for the torrent will be sent
    /// as `FoundGoodPiece` messages BEFORE this message is sent.
    TorrentAdded(InfoHash),
    /// Message indicating that the torrent has been removed.
    TorrentRemoved(InfoHash),
    /// Message indicating that the torrent has been synced.
    TorrentSynced(InfoHash),
    /// Message indicating that a good piece has been identified for
    /// the given torrent (hash), as well as the piece index.
    FoundGoodPiece(InfoHash, u64),
    /// Message indicating that a bad piece has been identified for
    /// the given torrent (hash), as well as the piece index.
    FoundBadPiece(InfoHash, u64),
    /// Message indicating that the given block has been loaded.
    BlockLoaded(Block),
    /// Message indicating that the given block has been processed.
    BlockProcessed(Block),
    /// Error occurring from a `AddTorrent` or `RemoveTorrent` message.
    TorrentError(InfoHash, TorrentError),
    /// Error occurring from a `LoadBlock` or `ProcessBlock` message.
    BlockError(Block, BlockError)
}