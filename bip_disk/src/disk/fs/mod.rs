use std::path::{Path};
use std::io::{self};

pub mod native;

/// Trait for performing operations on some file system.
///
/// Relative paths will originate from an implementation defined directory.
pub trait FileSystem {
    /// Some file object.
    type File;

    /// Open a file, create it if it does not exist.
    ///
    /// Intermediate directories will be created if necessary.
    fn open_file<P>(&self, path: P) -> io::Result<Self::File>
        where P: AsRef<Path> + Send + 'static;

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

    fn open_file<P>(&self, path: P) -> io::Result<Self::File>
        where P: AsRef<Path> + Send + 'static {
        FileSystem::open_file(*self, path)
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