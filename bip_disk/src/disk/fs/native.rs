use std::path::{Path, PathBuf};
use std::io::{self, Write, Read, Seek, SeekFrom};
use std::fs::{self, File, OpenOptions};
use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use bip_util::bt::InfoHash;
use bip_util::sha::ShaHashBuilder;
use bip_util::convert;
use bytes::{Buf, BufMut};
use futures_cpupool::{Builder, CpuPool, CpuFuture};

use disk::fs::FileSystem;
use memory::block::Block;

// TODO: This should be sanitizing paths passed into it so they don't escape the base directory!!!

/// File that exists on disk.
pub struct NativeFile {
    inner: Arc<InnerNativeFile>
}

struct InnerNativeFile {
    file: Mutex<File>,
    path: PathBuf
}

impl InnerNativeFile {
    fn new(file: File, path: PathBuf) -> InnerNativeFile {
        InnerNativeFile{ file: Mutex::new(file), path: path }
    }

    fn run_with_file<F, R>(&self, call: F) -> R
        where F: FnOnce(&mut File) -> R {
        let mut lock_file = self.file.lock()
            .expect("bip_disk: Failed To Lock NativeFile");

        call(&mut lock_file)
    }
}

impl NativeFile {
    /// Create a new NativeFile.
    fn new(file: File, path: PathBuf) -> NativeFile {
        NativeFile{ inner: Arc::new(InnerNativeFile::new(file, path)) }
    }
}

//----------------------------------------------------------------------------//

/// File system that maps to the OS file system.
pub struct NativeFileSystem {
    base_dir: Arc<PathBuf>,
    cpu_pool: CpuPool
}

impl NativeFileSystem {
    /// Initialize a `NativeFileSystem` with the given base directory, and `CpuPool` settings.
    pub fn new<P>(base_dir: P, mut pool_config: Builder) -> NativeFileSystem
        where P: Into<PathBuf> {
        NativeFileSystem{ base_dir: Arc::new(base_dir.into()), cpu_pool: pool_config.create() }
    }
}

impl FileSystem for NativeFileSystem {
    type File = NativeFile;

    type FileFuture  = CpuFuture<NativeFile, io::Error>;
    type SizeFuture  = CpuFuture<u64, io::Error>;
    type BlockFuture = CpuFuture<(Block, usize), (Block, io::Error)>;
    type UnitFuture  = CpuFuture<(), (NativeFile, io::Error)>;

    fn open_file<P>(&self, path: P) -> CpuFuture<NativeFile, io::Error>
        where P: AsRef<Path> + Send + 'static {
        let inner_base_dir = self.base_dir.clone();

        self.cpu_pool.spawn_fn(move || {
            let combine_path = combine_user_path(&path, &&inner_base_dir);
            let file = try!(create_new_file(&combine_path));

            Ok(NativeFile::new(file, combine_path.into_owned()))
        })
    }

    fn file_size(&self, file: &NativeFile) ->  CpuFuture<u64, io::Error> {
        let inner_file = file.inner.clone();

        self.cpu_pool.spawn_fn(move || {
            inner_file.run_with_file(|file| {
                file.metadata().map(|metadata| metadata.len())
            })
        })
    }

    fn remove_file(&self, file: NativeFile) -> CpuFuture<(), (NativeFile, io::Error)> {
        self.cpu_pool.spawn_fn(move || {
            fs::remove_file(&file.inner.path).map_err(|err| (file, err))
        })
    }

    fn read_file(&self, file: &mut NativeFile, offset: u64, mut buffer: Block) -> CpuFuture<(Block, usize), (Block, io::Error)> {
        let inner_file = file.inner.clone();

        self.cpu_pool.spawn_fn(move || {
            inner_file.run_with_file(|file| {
                let res_bytes_read = file.seek(SeekFrom::Start(offset))
                    .and_then(|_| {
                        file.read(&mut buffer[..])
                    });

                match res_bytes_read {
                    Ok(read) => Ok((buffer, read)),
                    Err(err) => Err((buffer, err))
                }
            })

        })
    }

    fn write_file(&self, file: &mut NativeFile, offset: u64, buffer: Block) -> CpuFuture<(Block, usize), (Block, io::Error)> {
        let inner_file = file.inner.clone();

        self.cpu_pool.spawn_fn(move || {
            inner_file.run_with_file(|file| {
                let res_bytes_written = file.seek(SeekFrom::Start(offset))
                    .and_then(|_| {
                        file.write(&buffer[..])
                    });
                
                match res_bytes_written {
                    Ok(written) => Ok((buffer, written)),
                    Err(err)    => Err((buffer, err))
                }
            })
        })
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

            OpenOptions::new().read(true).write(true).create(true).open(&path)
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