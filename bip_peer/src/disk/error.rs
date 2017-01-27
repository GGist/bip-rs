use std::path::PathBuf;
use std::io;

use bip_util::bt::InfoHash;

error_chain! {
    types {
        RequestError, RequestErrorKind, RequestResultExt, RequestResult;
    }
}

error_chain! {
    types {
        TorrentError, TorrentErrorKind, TorrentResultExt, TorrentResult;
    }

    foreign_links {
        Io(io::Error);
    }

    errors {
        ExistingFileSizeCheck {
            file_path:     String,
            expected_size: u64,
            actual_size:   u64
        } {
            description("Failed To Add Torrent Because Size Checker Failed For A File")
            display("Failed To Add Torrent Because Size Checker Failed For {} Where File Size Was {} But Should Have Been {}", file_path, actual_size, expected_size)
        }
        ExistingInfoHash {
            hash: InfoHash
        } {
            description("Failed To Add Torrent Because Another Torrent With The Same InfoHash Is Already Added")
            display("Failed To Add Torrent Because Another Torrent With The Same InfoHash {:?} Is Already Added", hash)
        }
    }
}