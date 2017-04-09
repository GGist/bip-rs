use std::path::{Path};
use std::io::{self};

use bytes::{Buf, BufMut};
use futures::Future;

use memory::block::Block;

pub mod memory;
pub mod native;

/// Trait for performing operations on some file system.
///
/// Relative paths will originate from an implementation defined directory.
pub trait FileSystem {
    /// Some file object.
    type File;
    
    type FileFuture:  Future<Item=Self::File,     Error=io::Error>;
    type SizeFuture:  Future<Item=u64,            Error=io::Error>;
    type BlockFuture: Future<Item=(Block, usize), Error=(Block, io::Error)>;
    type UnitFuture:  Future<Item=(),             Error=(Self::File, io::Error)>;

    /// Open a file, create it if it does not exist.
    ///
    /// Intermediate directories will be created if necessary.
    fn open_file<P>(&self, path: P) -> Self::FileFuture
        where P: AsRef<Path> + Send + 'static;

    /// Get the size of the file in bytes.
    fn file_size(&self, file: &Self::File) -> Self::SizeFuture;

    /// Remove a given file from the file system.
    fn remove_file(&self, file: Self::File) -> Self::UnitFuture;

    /// Read the contents of the file at the given offset.
    ///
    /// On success, return the number of bytes read.
    fn read_file(&self, file: &mut Self::File, offset: u64, buffer: Block) -> Self::BlockFuture;

    /// Write the contents of the file at the given offset.
    ///
    /// On success, return the number of bytes written. If offset is
    /// past the current size of the file, zeroes will be filled in.
    fn write_file(&self, file: &mut Self::File, offset: u64, buffer: Block) -> Self::BlockFuture;
}

impl<'a, F> FileSystem for &'a F where F: FileSystem {
    type File = F::File;

    type FileFuture  = F::FileFuture;
    type SizeFuture  = F::SizeFuture;
    type BlockFuture = F::BlockFuture;
    type UnitFuture  = F::UnitFuture;

    fn open_file<P>(&self, path: P) -> Self::FileFuture
        where P: AsRef<Path> + Send + 'static {
        FileSystem::open_file(*self, path)
    }

    fn file_size(&self, file: &Self::File) -> Self::SizeFuture {
        FileSystem::file_size(*self, file)
    }

    fn remove_file(&self, file: Self::File) -> Self::UnitFuture {
        FileSystem::remove_file(*self, file)
    }

    fn read_file(&self, file: &mut Self::File, offset: u64, buffer: Block) -> Self::BlockFuture {
        FileSystem::read_file(*self, file, offset, buffer)
    }

    fn write_file(&self, file: &mut Self::File, offset: u64, buffer: Block) -> Self::BlockFuture {
        FileSystem::write_file(*self, file, offset, buffer)
    }
}