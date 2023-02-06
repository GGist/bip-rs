use std::path::{Path, PathBuf};

use bip_metainfo::File;

pub mod piece_accessor;
pub mod piece_checker;

pub fn build_path(parent_directory: Option<&Path>, file: &File) -> PathBuf {
    match parent_directory {
        Some(dir) => PathBuf::from(dir).join(file.path()),
        None => file.path().to_owned(),
    }
}
