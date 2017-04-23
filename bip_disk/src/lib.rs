extern crate bip_metainfo;
extern crate bip_util;
extern crate crossbeam;
extern crate futures;
extern crate futures_cpupool;
#[macro_use]
extern crate error_chain;
extern crate tokio_core;

mod disk;
mod memory;
mod error;

pub use disk::{IDiskMessage, ODiskMessage};
pub use disk::fs::FileSystem;
pub use disk::builder::DiskManagerBuilder;
pub use disk::manager::{DiskManager, DiskManagerSink, DiskManagerStream};

pub use memory::manager::BlockManager;
pub use memory::block::{Block, BlockMetadata};

/// Built in objects implementing `FileSystem`.
pub mod fs {
    pub use disk::fs::native::{NativeFile, NativeFileSystem};
}