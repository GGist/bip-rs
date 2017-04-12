use disk::fs::FileSystem;
use disk::manager::{self, DiskManager};

use futures_cpupool::Builder;
use tokio_core::reactor::Handle;

const DEFAULT_PENDING_SIZE:   usize = 10;
const DEFAULT_COMPLETED_SIZE: usize = 10;

/// `DiskManagerBuilder` for building `DiskManager`s with different settings.
pub struct DiskManagerBuilder {
    builder:        Builder,
    pending_size:   usize,
    completed_size: usize
}

impl DiskManagerBuilder {
    pub fn new() -> DiskManagerBuilder {
        DiskManagerBuilder{ builder: Builder::new(), pending_size: DEFAULT_PENDING_SIZE,
                            completed_size: DEFAULT_COMPLETED_SIZE }
    }

    pub fn with_worker_config(&mut self, config: Builder) -> &mut DiskManagerBuilder {
        self.builder = config;
        self
    }

    pub fn with_pending_buffer_size(&mut self, size: usize) -> &mut DiskManagerBuilder {
        self.pending_size = size;
        self
    }

    pub fn with_completed_buffer_size(&mut self, size: usize) -> &mut DiskManagerBuilder {
        self.completed_size = size;
        self
    }

    pub fn build<F>(self, fs: F) -> DiskManager<F>
        where F: FileSystem {
        manager::new_manager(self.pending_size, self.completed_size, fs, self.builder)
    }
}
