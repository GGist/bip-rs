extern crate bip_metainfo;
extern crate bip_disk;
extern crate bip_util;
extern crate futures;
extern crate tokio_core;
extern crate rand;

use std::collections::HashMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Arc};
use std::cmp;

use bip_disk::FileSystem;
use bip_metainfo::{IntoAccessor, Accessor};
use rand::Rng;

mod add_torrent;
mod complete_torrent;
mod load_block;
mod process_block;
mod remove_torrent;

/// Generate buffer of size random bytes.
fn random_buffer(size: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; size];

    let mut rng = rand::weak_rng();
    for i in 0..size {
        buffer[i] = rng.gen();
    }

    buffer
}

/// Initiate a loop with the given core.
fn core_loop<>() -> 

//----------------------------------------------------------------------------//

/// Allow us to mock out multi file torrents.
struct MultiFileDirectAccessor {
    dir:   PathBuf,
    files: Vec<(Vec<u8>, PathBuf)>
}

impl MultiFileDirectAccessor {
    pub fn new(dir: PathBuf, files: Vec<(Vec<u8>, PathBuf)>) -> MultiFileDirectAccessor {
        MultiFileDirectAccessor{ dir: dir, files: files }
    }
}

// TODO: Ugh, once specialization lands, we can see about having a default impl for IntoAccessor
impl IntoAccessor for MultiFileDirectAccessor {
    type Accessor = MultiFileDirectAccessor;

    fn into_accessor(self) -> io::Result<MultiFileDirectAccessor> {
        Ok(self)
    }
}

impl Accessor for MultiFileDirectAccessor {
    fn access_directory(&self) -> Option<&Path> {
        // Do not just return the option here, unwrap it and put it in
        // another Option (since we know this is a multi file torrent)
        Some(self.dir.as_ref())
    }

    fn access_metadata<C>(&self, mut callback: C) -> io::Result<()>
        where C: FnMut(u64, &Path) {
        for &(ref buffer, ref path) in self.files.iter() {
            callback(buffer.len() as u64, &*path)
        }

        Ok(())
    }

    fn access_pieces<C>(&self, mut callback: C) -> io::Result<()>
        where C: FnMut(&mut Read) -> io::Result<()> {
        for &(ref buffer, _) in self.files.iter() {
            try!(callback(&mut &buffer[..]))
        }

        Ok(())
    }
}

//----------------------------------------------------------------------------//

/// Allow us to mock out the file system.
#[derive(Clone)]
struct InMemoryFileSystem {
    files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>
}

impl InMemoryFileSystem {
    pub fn new() -> InMemoryFileSystem {
        InMemoryFileSystem{ files: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn run_with_lock<C, R>(&self, call: C) -> R
        where C: FnOnce(&mut HashMap<PathBuf, Vec<u8>>) -> R {
        let mut lock_files = self.files.lock().unwrap();

        call(&mut *lock_files)
    }
}

struct InMemoryFile {
    path: PathBuf
}

impl FileSystem for InMemoryFileSystem {
    type File = InMemoryFile;

    fn open_file<P>(&self, path: P) -> io::Result<Self::File> 
        where P: AsRef<Path> + Send + 'static {
        let file_path = path.as_ref().to_path_buf();

        self.run_with_lock(|files| {
            if !files.contains_key(&file_path) {
                files.insert(file_path.clone(), Vec::new());
            }
        });

        Ok(InMemoryFile{ path: file_path })
    }

    fn file_size(&self, file: &Self::File) -> io::Result<u64> {
        self.run_with_lock(|files| {
            files.get(&file.path)
                .map(|file| file.len() as u64)
                .ok_or(io::Error::new(io::ErrorKind::NotFound, "File Not Found"))
        })
    }

    fn remove_file(&self, file: Self::File) -> io::Result<()> {
        self.run_with_lock(|files| {
            files.remove(&file.path)
                .map(|_| ())
                .ok_or(io::Error::new(io::ErrorKind::NotFound, "File Not Found"))
        })
    }

    fn read_file(&self, file: &mut Self::File, offset: u64, buffer: &mut [u8]) -> io::Result<usize> {
        self.run_with_lock(|files| {
            files.get(&file.path)
                .map(|file_buffer| {
                    let cast_offset = offset as usize;
                    let bytes_to_copy = cmp::min(file_buffer.len() - cast_offset, buffer.len());
                    let bytes = &file_buffer[cast_offset..(bytes_to_copy + cast_offset)];

                    buffer.clone_from_slice(bytes);

                    bytes_to_copy
                })
                .ok_or(io::Error::new(io::ErrorKind::NotFound, "File Not Found"))
        })
    }

    fn write_file(&self, file: &mut Self::File, offset: u64, buffer: &[u8]) -> io::Result<usize> {
        self.run_with_lock(|files| {
            files.get_mut(&file.path)
                .map(|file_buffer| {
                    let cast_offset = offset as usize;

                    let last_byte_pos = cast_offset + buffer.len();
                    if last_byte_pos > file_buffer.len() {
                        file_buffer.resize(last_byte_pos, 0);
                    }

                    let bytes_to_copy = cmp::min(file_buffer.len() - cast_offset, buffer.len());
                    
                    if bytes_to_copy != 0 {
                        file_buffer[cast_offset..(cast_offset + bytes_to_copy)].clone_from_slice(buffer);
                    }

                    // TODO: If the file is full, this will return zero, we should also simulate io::ErrorKind::WriteZero
                    bytes_to_copy
                })
                .ok_or(io::Error::new(io::ErrorKind::NotFound, "File Not Found"))
        })
    }
}
