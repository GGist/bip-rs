use crate::disk::fs::FileSystem;
use crate::disk::manager::DiskManager;

use futures_cpupool::Builder;

const DEFAULT_PENDING_SIZE: usize = 10;
const DEFAULT_COMPLETED_SIZE: usize = 10;

/// `DiskManagerBuilder` for building `DiskManager`s with different settings.
pub struct DiskManagerBuilder {
    builder: Builder,
    pending_size: usize,
    completed_size: usize,
}

impl DiskManagerBuilder {
    /// Create a new `DiskManagerBuilder`.
    pub fn new() -> DiskManagerBuilder {
        DiskManagerBuilder {
            builder: Builder::new(),
            pending_size: DEFAULT_PENDING_SIZE,
            completed_size: DEFAULT_COMPLETED_SIZE,
        }
    }

    /// Use a custom `Builder` for the `CpuPool`.
    pub fn with_worker_config(mut self, config: Builder) -> DiskManagerBuilder {
        self.builder = config;
        self
    }

    /// Specify the buffer capacity for pending `IDiskMessage`s.
    pub fn with_sink_buffer_capacity(mut self, size: usize) -> DiskManagerBuilder {
        self.pending_size = size;
        self
    }

    /// Specify the buffer capacity for completed `ODiskMessage`s.
    pub fn with_stream_buffer_capacity(mut self, size: usize) -> DiskManagerBuilder {
        self.completed_size = size;
        self
    }

    /// Retrieve the `CpuPool` builder.
    pub fn worker_config(&mut self) -> &mut Builder {
        &mut self.builder
    }

    /// Retrieve the sink buffer capacity.
    pub fn sink_buffer_capacity(&self) -> usize {
        self.pending_size
    }

    /// Retrieve the stream buffer capacity.
    pub fn stream_buffer_capacity(&self) -> usize {
        self.completed_size
    }

    /// Build a `DiskManager` with the given `FileSystem`.
    pub fn build<F>(self, fs: F) -> DiskManager<F>
    where
        F: FileSystem + Send + Sync + 'static,
    {
        DiskManager::from_builder(self, fs)
    }
}
