use std::time::Duration;
use std::io;

use manager::{PeerManager, ManagedMessage};

use futures::sink::Sink;
use futures::stream::Stream;
use tokio_core::reactor::Handle;

const DEFAULT_SINK_BUFFER_CAPACITY:      usize = 100;
const DEFAULT_STREAM_BUFFER_CAPACITY:    usize = 100;
const DEFAULT_HEARTBEAT_INTERVAL_MILLIS: u64   = 1 * 60 * 1000;
const DEFAULT_HEARTBEAT_TIMEOUT_MILLIS:  u64   = 2 * 60 * 1000;

/// Builder for configuring a `PeerManager`.
#[derive(Copy, Clone)]
pub struct PeerManagerBuilder {
    sink_buffer:        usize,
    stream_buffer:      usize,
    heartbeat_interval: Duration,
    heartbeat_timeout:  Duration
}

impl PeerManagerBuilder {
    /// Create a new `PeerManagerBuilder`.
    pub fn new() -> PeerManagerBuilder {
        PeerManagerBuilder {
            sink_buffer:        DEFAULT_SINK_BUFFER_CAPACITY,
            stream_buffer:      DEFAULT_STREAM_BUFFER_CAPACITY,
            heartbeat_interval: Duration::from_millis(DEFAULT_HEARTBEAT_INTERVAL_MILLIS),
            heartbeat_timeout:  Duration::from_millis(DEFAULT_HEARTBEAT_TIMEOUT_MILLIS)
        }
    }

    /// Capacity of pending sent messages.
    pub fn with_sink_buffer_capacity(mut self, capacity: usize) -> PeerManagerBuilder {
        self.sink_buffer = capacity;
        self
    }

    /// Capacity of pending received messages.
    pub fn with_stream_buffer_capacity(mut self, capacity: usize) -> PeerManagerBuilder {
        self.stream_buffer = capacity;
        self
    }

    /// Interval at which we send keep-alive messages.
    pub fn with_heartbeat_interval(mut self, interval: Duration) -> PeerManagerBuilder {
        self.heartbeat_interval = interval;
        self
    }

    /// Timeout at which we disconnect from the peer without seeing a keep-alive message.
    pub fn with_heartbeat_timeout(mut self, timeout: Duration) -> PeerManagerBuilder {
        self.heartbeat_timeout = timeout;
        self
    }

    /// Retrieve the sink buffer capacity.
    pub fn sink_buffer_capacity(&self) -> usize {
        self.sink_buffer
    }

    /// Retrieve the stream buffer capacity.
    pub fn stream_buffer_capacity(&self) -> usize {
        self.stream_buffer
    }

    /// Retrieve the hearbeat interval `Duration`.
    pub fn heartbeat_interval(&self) -> Duration {
        self.heartbeat_interval
    }

    /// Retrieve the heartbeat timeout `Duration`.
    pub fn heartbeat_timeout(&self) -> Duration {
        self.heartbeat_timeout
    }

    /// Build a `PeerManager` from the current `PeerManagerBuilder`.
    pub fn build<P>(self, handle: Handle) -> PeerManager<P>
        where P: Sink<SinkError=io::Error> +
                 Stream<Error=io::Error>,
              P::SinkItem: ManagedMessage,
              P::Item:     ManagedMessage {
        PeerManager::from_builder(self, handle)
    }
}