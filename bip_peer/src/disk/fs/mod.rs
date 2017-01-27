use std::path::{Path, PathBuf};
use std::io::{self, Write, Read, Seek, SeekFrom};
use std::fs::{self, File, OpenOptions};
use std::borrow::Cow;

use bip_util::bt::InfoHash;
use bip_util::sha::ShaHashBuilder;
use bip_util::convert;
use rand;

pub mod memory;
pub mod native;

/// Trait for performing operations on some file system.
///
/// Relative paths will originate from an implementation defined directory.
pub trait FileSystem {
    /// Some file object.
    type File;

    /// Open a file, create it if it does not exist.
    ///
    /// Intermediate directories will be created if necessary. If
    /// no path is given, a file with a random name will be created.
    fn open_file<P>(&self, opt_path: Option<P>) -> io::Result<Self::File>
        where P: AsRef<Path>;

    /// Get the size of the file in bytes.
    fn file_size(&self, file: &Self::File) -> io::Result<u64>;

    /// Remove a given file from the file system.
    fn remove_file(&self, file: Self::File) -> io::Result<()>;

    /// Read the contents of the file at the given offset.
    ///
    /// On success, return the number of bytes read.
    fn read_file(&self, file: &mut Self::File, offset: u64, buffer: &mut [u8]) -> io::Result<usize>;

    /// Write the contents of the file at the given offset.
    ///
    /// On success, return the number of bytes written. If offset is
    /// past the current size of the file, zeroes will be filled in.
    fn write_file(&self, file: &mut Self::File, offset: u64, buffer: &[u8]) -> io::Result<usize>;
}

impl<'a, F> FileSystem for &'a F where F: FileSystem {
    type File = F::File;

    fn open_file<P>(&self, opt_path: Option<P>) -> io::Result<Self::File>
        where P: AsRef<Path> {
        FileSystem::open_file(*self, opt_path)
    }

    fn file_size(&self, file: &Self::File) -> io::Result<u64> {
        FileSystem::file_size(*self, file)
    }

    fn remove_file(&self, file: Self::File) -> io::Result<()> {
        FileSystem::remove_file(*self, file)
    }

    fn read_file(&self, file: &mut Self::File, offset: u64, buffer: &mut [u8]) -> io::Result<usize> {
        FileSystem::read_file(*self, file, offset, buffer)
    }

    fn write_file(&self, file: &mut Self::File, offset: u64, buffer: &[u8]) -> io::Result<usize> {
        FileSystem::write_file(*self, file, offset, buffer)
    }
}