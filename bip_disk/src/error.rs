use std::io;
use std::path::PathBuf;

use bip_util::bt::InfoHash;

error_chain! {
    types {
        BlockError, BlockErrorKind, BlockResultExt, BlockResult;
    }

    foreign_links {
        Io(io::Error);
    }

    errors {
        InfoHashNotFound {
            hash: InfoHash
        } {
            description("Failed To Load/Process Block Because Torrent Is Not Loaded")
            display("Failed To Load/Process Block Because The InfoHash {:?} It Is Not Currently Added", hash)
        }
    }
}

error_chain! {
    types {
        TorrentError, TorrentErrorKind, TorrentResultExt, TorrentResult;
    }

    foreign_links {
        Block(BlockError);
        Io(io::Error);
    }

    errors {
        ExistingFileSizeCheck {
            file_path:     PathBuf,
            expected_size: u64,
            actual_size:   u64
        } {
            description("Failed To Add Torrent Because Size Checker Failed For A File")
            display("Failed To Add Torrent Because Size Checker Failed For {:?} Where File Size Was {} But Should Have Been {}", file_path, actual_size, expected_size)
        }
        ExistingInfoHash {
            hash: InfoHash
        } {
            description("Failed To Add Torrent Because Another Torrent With The Same InfoHash Is Already Added")
            display("Failed To Add Torrent Because Another Torrent With The Same InfoHash {:?} Is Already Added", hash)
        }
        InfoHashNotFound {
            hash: InfoHash
        } {
            description("Failed To Remove Torrent Because It Is Not Currently Added")
            display("Failed To Remove Torrent Because The InfoHash {:?} It Is Not Currently Added", hash)
        }
    }
}
