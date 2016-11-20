use std::path::{Path, PathBuf};
use std::io::{self, Write, Read, Seek, SeekFrom};
use std::fs::{self, File, OpenOptions};
use std::borrow::Cow;

use bip_util::bt::InfoHash;
use bip_util::sha::ShaHashBuilder;
use bip_util::convert;
use rand;

/// Trait for performing operations on some file system.
///
/// Relative paths will originate from an implementation defined directory.
pub trait FileSystem {
    /// Some file object.
    type File;

    /// Create a file at the given path.
    ///
    /// Intermediate directories will be created if necessary. If
    /// no path is given, a file with a random name will be created.
    fn create_file<P>(&self, opt_path: Option<P>) -> io::Result<Self::File>
        where P: AsRef<Path>;

    /// Remove a given file from the file system.
    fn remove_file(&self, file: Self::File) -> io::Result<()>;

    /// Create a directory at the given path.
    ///
    /// Fails if the directory already exists.
    fn create_directory<P>(&self, path: P) -> io::Result<()>
        where P: AsRef<Path>;

    /// Read the contents of the file at the given offset.
    ///
    /// On success, return the number of bytes read.
    fn read_file(&self, file: &mut Self::File, offset: u64, buffer: &mut [u8]) -> io::Result<usize>;

    /// Write the contents of the file at the given offset.
    ///
    /// On success, return the number of bytes written.
    fn write_file(&self, file: &mut Self::File, offset: u64, buffer: &[u8]) -> io::Result<usize>;
}

// ----------------------------------------------------------------------------//

// TODO: This should be sanitizing paths passed into it!!!

/// File system that maps to the OS file system.
pub struct OSFileSystem {
    current_dir: PathBuf
}

/// File that exists on disk.
pub struct OSFile {
    file: File,
    path: PathBuf,
}

impl OSFile {
    /// Create a new OSFile.
    fn new(file: File, path: PathBuf) -> OSFile {
        OSFile{ file: file, path: path }
    }
}

impl OSFileSystem {
    /// Initialize a new OSFileSystem with the default directory set.
    pub fn with_directory<P>(default: P) -> OSFileSystem
        where P: AsRef<Path> {
        OSFileSystem{ current_dir: default.as_ref().to_path_buf() }
    }

    /// Create a scratch file with a random name in the current directory.
    fn create_scratch_file(&self) -> io::Result<OSFile> {
        let init_seed = rand::random::<u32>();
        let mut init_hash = ShaHashBuilder::new()
            .add_bytes(&convert::four_bytes_to_array(init_seed))
            .build();

        // Keep rehashing our hex string until we get a not in use file name
        loop {
            let hex_hash = info_hash_to_hex(init_hash);
            let scratch_path = combine_user_path(&hex_hash, &self.current_dir);
            
            match create_new_file(&scratch_path) {
                Ok(scratch_file) => { return Ok(OSFile::new(scratch_file, scratch_path.into_owned())) },
                Err(ref error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    init_hash = ShaHashBuilder::new()
                        .add_bytes(init_hash.as_ref())
                        .build();
                },
                Err(error) => { return Err(error) }
            }
        }
    }
}

/// Create a hex representation of the given InfoHash.
fn info_hash_to_hex(hash: InfoHash) -> String {
    let hex_len = hash.as_ref().len() * 2;

    hash.as_ref()
        .iter()
        .map(|b| format!("{:02X}", b))
        .fold(String::with_capacity(hex_len), |mut acc, nex| {
            acc.push_str(&nex);
            acc
        })
}

impl FileSystem for OSFileSystem {
    type File = OSFile;

    fn create_file<P>(&self, opt_path: Option<P>) -> io::Result<OSFile>
        where P: AsRef<Path> {
        match opt_path {
            Some(path) => {
                let combine_path = combine_user_path(&path, &self.current_dir);
                let file = try!(create_new_file(&combine_path));

                Ok(OSFile::new(file, combine_path.into_owned()))
            },
            None => {
                self.create_scratch_file()
            }
        }
    }

    fn remove_file(&self, file: OSFile) -> io::Result<()> {
        fs::remove_file(&file.path)
    }

    fn create_directory<P>(&self, path: P) -> io::Result<()>
        where P: AsRef<Path> {
        let combine_path = combine_user_path(&path, &self.current_dir);
        
        fs::create_dir(&combine_path)
    }

    fn read_file(&self, file: &mut OSFile, offset: u64, buffer: &mut [u8]) -> io::Result<usize> {
        try!(file.file.seek(SeekFrom::Start(offset)));

        file.file.read(buffer)
    }

    fn write_file(&self, file: &mut OSFile, offset: u64, buffer: &[u8]) -> io::Result<usize> {
        try!(file.file.seek(SeekFrom::Start(offset)));

        file.file.write(buffer)
    }
}

/// Create a new file with read and write options.
///
/// Intermediate directories will be created if they do not exist.
fn create_new_file<P>(path: P) -> io::Result<File>
    where P: AsRef<Path> {
    match path.as_ref().parent() {
        Some(parent_dir) => {
            try!(fs::create_dir_all(parent_dir));

            OpenOptions::new().read(true).write(true).create_new(true).open(&path)
        },
        None => {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "File Path Has No Parent"))
        }
    }
}

/// Create a path from the user path and current directory.
fn combine_user_path<'a, P>(user_path: &'a P, current_dir: &Path) -> Cow<'a, Path>
    where P: AsRef<Path> {
    let ref_user_path = user_path.as_ref();

    if ref_user_path.is_absolute() {
        Cow::Borrowed(ref_user_path)
    } else {
        let mut combine_user_path = current_dir.to_path_buf();

        for user_path_piece in ref_user_path.iter() {
            combine_user_path.push(user_path_piece);
        }
        
        Cow::Owned(combine_user_path)
    }
}

// ----------------------------------------------------------------------------//

/// File system that stores all data in memory.
pub struct MemFileSystem { }