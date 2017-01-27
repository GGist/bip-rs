use std::io;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::path::{PathBuf, Path};
use std::cmp;
use std::cell::RefCell;

use bip_metainfo::{MetainfoFile, InfoDictionary, File};
use bip_util::bt::InfoHash;
use bip_util::send::TrySender;
use chan::{self, Sender, Receiver};

use disk::worker::shared::blocks::Blocks;
use disk::worker::shared::clients::Clients;
use disk::error::{TorrentResult, TorrentError, TorrentErrorKind};
use disk::{IDiskMessage, ODiskMessage};
use disk::worker::{self, ReserveBlockClientMetadata, SyncBlockMessage, AsyncBlockMessage, DiskMessage};
use disk::worker::disk_worker::context::DiskWorkerContext;
use disk::fs::{FileSystem};
use token::{Token, TokenGenerator};
use message::standard::PieceMessage;

pub struct PieceReader<'a, F> {
    fs: F,
    info_dict: &'a InfoDictionary
}

impl<'a, F> PieceReader<'a, F> where F: FileSystem {
    pub fn new(fs: F, info_dict: &'a InfoDictionary) -> PieceReader<'a, F> {
        PieceReader{
            fs: fs,
            info_dict: info_dict
        }
    }

    pub fn read_piece(&self, piece_buffer: &mut [u8], message: &PieceMessage) -> TorrentResult<()> {
        let piece_length = self.info_dict.piece_length() as u64;

        let mut total_bytes_to_skip = message.piece_index() as u64 * piece_length;
        let mut total_bytes_read = 0;

        for file in self.info_dict.files() {
            let total_file_size = file.length() as u64;

            let mut bytes_to_read = total_file_size;
            let min_bytes_to_skip = cmp::min(total_bytes_to_skip, bytes_to_read);

            total_bytes_to_skip -= min_bytes_to_skip;
            bytes_to_read -= min_bytes_to_skip;

            if bytes_to_read > 0 && total_bytes_read < piece_length {
                let file_path = build_path(self.info_dict.directory(), file);
                let mut fs_file = try!(self.fs.open_file(Some(file_path)));

                let total_max_bytes_to_read = piece_length - total_bytes_read;
                let actual_bytes_to_read = cmp::min(total_max_bytes_to_read, bytes_to_read);
                let offset = total_file_size - bytes_to_read;
                    
                let (begin, end) = (total_bytes_read as usize, (total_bytes_read + actual_bytes_to_read) as usize);
                try!(self.fs.read_file(&mut fs_file, offset, &mut piece_buffer[begin..end]));
                total_bytes_read += actual_bytes_to_read;
            }
        }

        Ok(())
    }
}

fn build_path(parent_directory: Option<&str>, file: &File) -> String {
    let parent_directory = parent_directory.unwrap_or(".");

    file.paths().fold(parent_directory.to_string(), |mut acc, item| {
        acc.push_str("/");
        acc.push_str(item);

        acc
    })
}