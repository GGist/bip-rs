use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::disk::fs::FileSystem;

use lru_cache::LruCache;

/// Caches file handles to prevent going to the OS for every call to open a
/// file.
///
/// This is especially useful for consumer computers that have anti-virus
/// software installed, which will significantly increase the cost for opening
/// any files (with windows built in anti virus, I saw 20x slow downs).
pub struct FileHandleCache<F>
where
    F: FileSystem,
{
    cache: Mutex<LruCache<PathBuf, Arc<Mutex<F::File>>>>,
    inner: F,
}

impl<F> FileHandleCache<F>
where
    F: FileSystem,
{
    /// Create a new `FileHandleCache` with the given handle capacity and an
    /// inner `FileSystem` which will be called for handles not in the cache.
    pub fn new(inner: F, capacity: usize) -> FileHandleCache<F> {
        FileHandleCache {
            cache: Mutex::new(LruCache::new(capacity)),
            inner,
        }
    }

    fn run_with_lock<C, R>(&self, call: C) -> R
    where
        C: FnOnce(&mut LruCache<PathBuf, Arc<Mutex<F::File>>>, &F) -> R,
    {
        let mut lock_cache = self
            .cache
            .lock()
            .expect("bip_disk: Failed To Lock Cache In FileHandleCache::run_with_lock");

        call(&mut *lock_cache, &self.inner)
    }
}

impl<F> FileSystem for FileHandleCache<F>
where
    F: FileSystem,
{
    type File = Arc<Mutex<F::File>>;

    fn open_file<P>(&self, path: P) -> io::Result<Self::File>
    where
        P: AsRef<Path> + Send + 'static,
    {
        self.run_with_lock(|cache, fs| {
            {
                if let Some(entry) = cache.get_mut(path.as_ref()) {
                    return Ok(entry.clone());
                }
            }
            let path_buf = path.as_ref().to_path_buf();
            let file = Arc::new(Mutex::new(fs.open_file(path)?));

            cache.insert(path_buf, file.clone());

            Ok(file)
        })
    }

    fn sync_file<P>(&self, path: P) -> io::Result<()>
    where
        P: AsRef<Path> + Send + 'static,
    {
        self.run_with_lock(|cache, _| cache.clear());

        self.inner.sync_file(path)
    }

    fn file_size(&self, file: &Self::File) -> io::Result<u64> {
        let lock_file = file
            .lock()
            .expect("bip_disk: Failed To Lock File In FileHandleCache::file_size");

        self.inner.file_size(&*lock_file)
    }

    fn read_file(
        &self,
        file: &mut Self::File,
        offset: u64,
        buffer: &mut [u8],
    ) -> io::Result<usize> {
        let mut lock_file = file
            .lock()
            .expect("bip_disk: Failed To Lock File In FileHandleCache::read_file");

        self.inner.read_file(&mut *lock_file, offset, buffer)
    }

    fn write_file(&self, file: &mut Self::File, offset: u64, buffer: &[u8]) -> io::Result<usize> {
        let mut lock_file = file
            .lock()
            .expect("bip_disk: Failed To Lock File In FileHandleCache::write_file");

        self.inner.write_file(&mut *lock_file, offset, buffer)
    }
}
