use disk::fs::FileSystem;
use disk::manager::{self, DiskManager};

use futures_cpupool::Builder;

const DEFAULT_PENDING_SIZE:   usize = 10;
const DEFAULT_COMPLETED_SIZE: usize = 10;

/// `DiskManagerBuilder` for building `DiskManager`s with different settings.
pub struct DiskManagerBuilder {
    builder:        Builder,
    pending_size:   usize,
    completed_size: usize
}

impl DiskManagerBuilder {
    /// Create a new `DiskManagerBuilder`.
    pub fn new() -> DiskManagerBuilder {
        DiskManagerBuilder{ builder: Builder::new(), pending_size: DEFAULT_PENDING_SIZE,
                            completed_size: DEFAULT_COMPLETED_SIZE }
    }

    /// Use a custom `Builder` for the `CpuPool`.
    pub fn with_worker_config(mut self, config: Builder) -> DiskManagerBuilder {
        self.builder = config;
        self
    }

    /// Specify the buffer size for pending `IDiskMessage`s.
    pub fn with_pending_buffer_size(mut self, size: usize) -> DiskManagerBuilder {
        self.pending_size = size;
        self
    }

    /// Specify the buffer size for completed `ODiskMessage`s.
    pub fn with_completed_buffer_size(mut self, size: usize) -> DiskManagerBuilder {
        self.completed_size = size;
        self
    }

    /// Build a `DiskManager` with the given `FileSystem`.
    pub fn build<F>(self, fs: F) -> DiskManager<F>
        where F: FileSystem + Send + Sync + 'static {
        manager::new_manager(self.pending_size, self.completed_size, fs, self.builder)
    }
}
