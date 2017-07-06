use std::io;
use std::time::Duration;

use tokio_timer::{Timer, TimeoutError, Sleep};
use futures::{Poll, Async, Future};
use futures::stream::{Stream, Fuse};

/// Error type for `PersistentStream`.
pub enum PersistentError {
    Disconnect,
    Timeout,
    IoError(io::Error)
}

impl<T> From<TimeoutError<T>> for PersistentError {
    fn from(error: TimeoutError<T>) -> PersistentError {
        match error {
            TimeoutError::Timer(_, _) => panic!("bip_peer: Timer Error In Peer Stream, Timer Capacity Is Probably Too Small..."),
            TimeoutError::TimedOut(_) => PersistentError::Timeout
        }
    }
}

/// Stream for persistent connections, where a value of None from the underlying
/// stream maps to an actual error, and calling poll multiple times will always
/// return such error.
pub struct PersistentStream<S> {
    stream: Fuse<S>
}

impl<S> PersistentStream<S> where S: Stream {
    /// Create a new `PersistentStream`.
    pub fn new(stream: S) -> PersistentStream<S> {
        PersistentStream{ stream: stream.fuse() }
    }
}

impl<S> Stream for PersistentStream<S>
    where S: Stream<Error=io::Error> {
    type Item = S::Item;
    type Error = PersistentError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.stream.poll()
            .map_err(|error| PersistentError::IoError(error))
            .and_then(|item| {
                match item {
                    Async::Ready(None) => Err(PersistentError::Disconnect),
                    other @ _          => Ok(other)
                }
            })
    }
}

//----------------------------------------------------------------------------//

/// Error type for `RecurringTimeoutStream`.
pub enum RecurringTimeoutError {
    /// None and any errors are mapped to this type...
    Disconnect,
    Timeout
}

/// Stream similar to `tokio_timer::TimeoutStream`, but which doesn't return
/// the underlying stream if a single timeout occurs. Instead, it signals that
/// the timeout occurred before the stream produced an item, but keeps the
/// stream in tact (does not return it), so that we can continue polling.
///
/// Whereas `tokio_timer::TimeoutStream` would be used for detecting if a
/// client timed out, `RecurringTimeoutStream` could be used for a local
/// stream to send heartbeats if, for example, the local client hadnt sent
/// any other message to the client for n seconds and we would like to send
/// some heartbeat message in that case, but continue polling the stream.
pub struct RecurringTimeoutStream<S> {
    dur:    Duration,
    timer:  Timer,
    sleep:  Sleep,
    stream: S
}

impl<S> RecurringTimeoutStream<S> {
    pub fn new(stream: S, timer: Timer, dur: Duration) -> RecurringTimeoutStream<S> {
        let sleep = timer.sleep(dur);

        RecurringTimeoutStream{ dur: dur, timer: timer, sleep: sleep, stream: stream }
    }
}

impl<S> Stream for RecurringTimeoutStream<S>
    where S: Stream
{
    type Item = S::Item;
    type Error = RecurringTimeoutError;

    fn poll(&mut self) -> Poll<Option<S::Item>, RecurringTimeoutError> {
        // First, try polling the future
        match self.stream.poll() {
            Ok(Async::NotReady) => {},
            Ok(Async::Ready(Some(v))) => {
                // Reset the timeout
                self.sleep = self.timer.sleep(self.dur);

                // Return the value
                return Ok(Async::Ready(Some(v)));
            },
            Ok(Async::Ready(None)) => { return Ok(Async::Ready(None)) },
            Err(_) => { return Err(RecurringTimeoutError::Disconnect) }
        }

        // Now check the timer
        match self.sleep.poll() {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(_)) => {
                // Reset the timeout
                self.sleep = self.timer.sleep(self.dur);
                
                Err(RecurringTimeoutError::Timeout)
            }
            Err(_) => panic!("bip_peer: Timer Error In Manager Stream, Timer Capacity Is Probably Too Small...")
        }
    }
}