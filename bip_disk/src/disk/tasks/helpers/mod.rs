use std::path::PathBuf;

use bip_metainfo::File;

pub mod piece_accessor;
pub mod piece_checker;

fn build_path(parent_directory: Option<&str>, file: &File) -> PathBuf {
    match parent_directory {
        Some(dir) => PathBuf::from(dir).join(file.path()),
        None      => file.path().to_owned()
    }
}