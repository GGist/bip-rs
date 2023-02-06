use std::default::Default;
use std::time::Duration;

const DEFAULT_HANDSHAKE_BUFFER_SIZE: usize = 1000;
const DEFAULT_WAIT_BUFFER_SIZE: usize = 10;
const DEFAULT_DONE_BUFFER_SIZE: usize = 10;

/// Once we get parallel handshake support (requires
/// mpmc future channel support, we can bump this up).
const DEFAULT_HANDSHAKE_TIMEOUT_MILLIS: u64 = 1000;
const DEFAULT_HANDSHAKE_CONNECT_TIMEOUT_MILLIS: u64 = 1000;

/// Configures the internals of a `Handshaker`.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct HandshakerConfig {
    sink_buffer_size: usize,
    wait_buffer_size: usize,
    done_buffer_size: usize,
    handshake_timeout: Duration,
    connect_timeout: Duration,
}

impl HandshakerConfig {
    /// Sets the buffer size that the `HandshakeSink` uses internally
    /// to hold `InitiateMessage`s before they are processed.
    pub fn with_sink_buffer_size(mut self, size: usize) -> HandshakerConfig {
        self.sink_buffer_size = size;
        self
    }

    /// Sets the buffer size that `Handshaker` uses internally
    /// to store handshake connections before they are processed.
    pub fn with_wait_buffer_size(mut self, size: usize) -> HandshakerConfig {
        self.wait_buffer_size = size;
        self
    }

    /// Sets the buffer size that `HandshakeStream` uses internally
    /// to store processed handshake connections before they are yielded.
    pub fn with_done_buffer_size(mut self, size: usize) -> HandshakerConfig {
        self.done_buffer_size = size;
        self
    }

    /// Sets the handshake timeout that `Handshaker` uses to
    /// make sure peers dont take too long to respond to us.
    pub fn with_handshake_timeout(mut self, timeout: Duration) -> HandshakerConfig {
        self.handshake_timeout = timeout;
        self
    }

    /// Sets the connect timeout that `Handshaker` uses to
    /// make sure peers dont take too long to respond to our
    /// connection (regardless of the underlying transport).
    pub fn with_connect_timeout(mut self, timeout: Duration) -> HandshakerConfig {
        self.connect_timeout = timeout;
        self
    }

    /// Gets the sink buffer size.
    pub fn sink_buffer_size(&self) -> usize {
        self.sink_buffer_size
    }

    /// Gets the wait buffer size.
    pub fn wait_buffer_size(&self) -> usize {
        self.wait_buffer_size
    }

    /// Gets the done buffer size.
    pub fn done_buffer_size(&self) -> usize {
        self.done_buffer_size
    }

    /// Gets the handshake timeout.
    pub fn handshake_timeout(&self) -> Duration {
        self.handshake_timeout
    }

    /// Gets the handshake connection initiation timeout.
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }
}

impl Default for HandshakerConfig {
    fn default() -> HandshakerConfig {
        HandshakerConfig {
            sink_buffer_size: DEFAULT_HANDSHAKE_BUFFER_SIZE,
            wait_buffer_size: DEFAULT_WAIT_BUFFER_SIZE,
            done_buffer_size: DEFAULT_DONE_BUFFER_SIZE,
            handshake_timeout: Duration::from_millis(DEFAULT_HANDSHAKE_TIMEOUT_MILLIS),
            connect_timeout: Duration::from_millis(DEFAULT_HANDSHAKE_CONNECT_TIMEOUT_MILLIS),
        }
    }
}
