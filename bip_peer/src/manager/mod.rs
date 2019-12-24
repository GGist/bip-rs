use std::cmp;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::manager::builder::PeerManagerBuilder;
use crate::manager::error::{PeerManagerError, PeerManagerErrorKind};
use crate::manager::peer_info::PeerInfo;

use crossbeam::sync::MsQueue;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::sync::mpsc::{self, Receiver, Sender};
use futures::task::{self as futures_task, Task};
use futures::{Async, AsyncSink, Poll, StartSend};
use tokio_core::reactor::Handle;
use tokio_timer::{self, Timer};

pub mod builder;
pub mod error;
pub mod peer_info;

mod future;
mod task;

// We configure our tick duration based on this, could let users configure this in the future...
const DEFAULT_TIMER_SLOTS: usize = 2048;

/// Manages a set of peers with heartbeating heartbeating.
pub struct PeerManager<P>
where
    P: Sink + Stream,
{
    sink: PeerManagerSink<P>,
    stream: PeerManagerStream<P>,
}

impl<P> PeerManager<P>
where
    P: Sink<SinkError = io::Error> + Stream<Error = io::Error>,
    P::SinkItem: ManagedMessage,
    P::Item: ManagedMessage,
{
    /// Create a new `PeerManager` from the given `PeerManagerBuilder`.
    pub fn from_builder(builder: PeerManagerBuilder, handle: Handle) -> PeerManager<P> {
        // We use one timer for manager heartbeat intervals, and one for peer heartbeat timeouts
        let maximum_timers = builder.peer_capacity() * 2;
        let pow_maximum_timers = if maximum_timers & (maximum_timers - 1) == 0 {
            maximum_timers
        } else {
            maximum_timers.next_power_of_two()
        };

        // Figure out the right tick duration to get num slots of 2048.
        // TODO: We could probably let users change this in the future...
        let max_duration = cmp::max(builder.heartbeat_interval(), builder.heartbeat_timeout());
        let tick_duration = Duration::from_millis(max_duration.as_secs() * 1000 / (DEFAULT_TIMER_SLOTS as u64) + 1);
        let timer = tokio_timer::wheel()
            .tick_duration(tick_duration)
            .max_capacity(pow_maximum_timers + 1)
            .channel_capacity(pow_maximum_timers)
            .num_slots(DEFAULT_TIMER_SLOTS)
            .build();

        let (res_send, res_recv) = mpsc::channel(builder.stream_buffer_capacity());
        let peers = Arc::new(Mutex::new(HashMap::new()));
        let task_queue = Arc::new(MsQueue::new());

        let sink = PeerManagerSink::new(handle, timer, builder, res_send, peers.clone(), task_queue.clone());
        let stream = PeerManagerStream::new(res_recv, peers, task_queue);

        PeerManager { sink, stream }
    }

    /// Break the `PeerManager` into a sink and stream.
    ///
    /// The returned sink implements `Clone`.
    pub fn into_parts(self) -> (PeerManagerSink<P>, PeerManagerStream<P>) {
        (self.sink, self.stream)
    }
}

impl<P> Sink for PeerManager<P>
where
    P: Sink<SinkError = io::Error> + Stream<Error = io::Error> + 'static,
    P::SinkItem: ManagedMessage,
    P::Item: ManagedMessage,
{
    type SinkItem = IPeerManagerMessage<P>;
    type SinkError = PeerManagerError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.sink.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.sink.poll_complete()
    }
}

impl<P> Stream for PeerManager<P>
where
    P: Sink + Stream,
{
    type Item = OPeerManagerMessage<P::Item>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.stream.poll()
    }
}

//----------------------------------------------------------------------------//

/// Sink half of a `PeerManager`.
pub struct PeerManagerSink<P>
where
    P: Sink + Stream,
{
    handle: Handle,
    timer: Timer,
    build: PeerManagerBuilder,
    send: Sender<OPeerManagerMessage<P::Item>>,
    peers: Arc<Mutex<HashMap<PeerInfo, Sender<IPeerManagerMessage<P>>>>>,
    task_queue: Arc<MsQueue<Task>>,
}

impl<P> Clone for PeerManagerSink<P>
where
    P: Sink + Stream,
{
    fn clone(&self) -> PeerManagerSink<P> {
        PeerManagerSink {
            handle: self.handle.clone(),
            timer: self.timer.clone(),
            build: self.build,
            send: self.send.clone(),
            peers: self.peers.clone(),
            task_queue: self.task_queue.clone(),
        }
    }
}

impl<P> PeerManagerSink<P>
where
    P: Sink + Stream,
{
    fn new(
        handle: Handle,
        timer: Timer,
        build: PeerManagerBuilder,
        send: Sender<OPeerManagerMessage<P::Item>>,
        peers: Arc<Mutex<HashMap<PeerInfo, Sender<IPeerManagerMessage<P>>>>>,
        task_queue: Arc<MsQueue<Task>>,
    ) -> PeerManagerSink<P> {
        PeerManagerSink {
            handle,
            timer,
            build,
            send,
            peers,
            task_queue,
        }
    }

    fn run_with_lock_sink<F, T, E, G, I>(&mut self, item: I, call: F, not: G) -> StartSend<T, E>
    where
        F: FnOnce(
            I,
            &mut Handle,
            &mut Timer,
            &mut PeerManagerBuilder,
            &mut Sender<OPeerManagerMessage<P::Item>>,
            &mut HashMap<PeerInfo, Sender<IPeerManagerMessage<P>>>,
        ) -> StartSend<T, E>,
        G: FnOnce(I) -> T,
    {
        let (result, took_lock) = if let Ok(mut guard) = self.peers.try_lock() {
            let result = call(
                item,
                &mut self.handle,
                &mut self.timer,
                &mut self.build,
                &mut self.send,
                &mut *guard,
            );

            // Closure could return not ready, need to stash in that case
            if result.as_ref().map(|r#async| r#async.is_not_ready()).unwrap_or(false) {
                self.task_queue.push(futures_task::current());
            }

            (result, true)
        } else {
            self.task_queue.push(futures_task::current());

            if let Ok(mut guard) = self.peers.try_lock() {
                let result = call(
                    item,
                    &mut self.handle,
                    &mut self.timer,
                    &mut self.build,
                    &mut self.send,
                    &mut *guard,
                );

                // Closure could return not ready, need to stash in that case
                if result.as_ref().map(|r#async| r#async.is_not_ready()).unwrap_or(false) {
                    self.task_queue.push(futures_task::current());
                }

                (result, true)
            } else {
                (Ok(AsyncSink::NotReady(not(item))), false)
            }
        };

        if took_lock {
            // Just notify a single person waiting on the lock to reduce contention
            if let Some(task) = self.task_queue.try_pop() {
                task.notify()
            }
        }

        result
    }

    fn run_with_lock_poll<F, T, E>(&mut self, call: F) -> Poll<T, E>
    where
        F: FnOnce(
            &mut Handle,
            &mut Timer,
            &mut PeerManagerBuilder,
            &mut Sender<OPeerManagerMessage<P::Item>>,
            &mut HashMap<PeerInfo, Sender<IPeerManagerMessage<P>>>,
        ) -> Poll<T, E>,
    {
        let (result, took_lock) = if let Ok(mut guard) = self.peers.try_lock() {
            let result = call(&mut self.handle, &mut self.timer, &mut self.build, &mut self.send, &mut *guard);

            (result, true)
        } else {
            // Stash a task
            self.task_queue.push(futures_task::current());

            // Try to get lock again in case of race condition
            if let Ok(mut guard) = self.peers.try_lock() {
                let result = call(&mut self.handle, &mut self.timer, &mut self.build, &mut self.send, &mut *guard);

                (result, true)
            } else {
                (Ok(Async::NotReady), false)
            }
        };

        if took_lock {
            // Just notify a single person waiting on the lock to reduce contention
            if let Some(task) = self.task_queue.try_pop() {
                task.notify()
            }
        }

        result
    }
}

impl<P> Sink for PeerManagerSink<P>
where
    P: Sink<SinkError = io::Error> + Stream<Error = io::Error> + 'static,
    P::SinkItem: ManagedMessage,
    P::Item: ManagedMessage,
{
    type SinkItem = IPeerManagerMessage<P>;
    type SinkError = PeerManagerError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match item {
            IPeerManagerMessage::AddPeer(info, peer) => self.run_with_lock_sink(
                (info, peer),
                |(info, peer), handle, timer, builder, send, peers| {
                    if peers.len() >= builder.peer_capacity() {
                        Ok(AsyncSink::NotReady(IPeerManagerMessage::AddPeer(info, peer)))
                    } else {
                        match peers.entry(info) {
                            Entry::Occupied(_) => Err(PeerManagerError::from_kind(PeerManagerErrorKind::PeerNotFound { info })),
                            Entry::Vacant(vac) => {
                                vac.insert(task::run_peer(peer, info, send.clone(), timer.clone(), builder, handle));

                                Ok(AsyncSink::Ready)
                            }
                        }
                    }
                },
                |(info, peer)| IPeerManagerMessage::AddPeer(info, peer),
            ),
            IPeerManagerMessage::RemovePeer(info) => self.run_with_lock_sink(
                info,
                |info, _, _, _, _, peers| {
                    peers
                        .get_mut(&info)
                        .ok_or_else(|| PeerManagerError::from_kind(PeerManagerErrorKind::PeerNotFound { info }))
                        .and_then(|send| {
                            send.start_send(IPeerManagerMessage::RemovePeer(info))
                                .map_err(|_| panic!("bip_peer: PeerManager Failed To Send RemovePeer"))
                        })
                },
                IPeerManagerMessage::RemovePeer,
            ),
            IPeerManagerMessage::SendMessage(info, mid, peer_message) => self.run_with_lock_sink(
                (info, mid, peer_message),
                |(info, mid, peer_message), _, _, _, _, peers| {
                    peers
                        .get_mut(&info)
                        .ok_or_else(|| PeerManagerError::from_kind(PeerManagerErrorKind::PeerNotFound { info }))
                        .and_then(|send| {
                            send.start_send(IPeerManagerMessage::SendMessage(info, mid, peer_message))
                                .map_err(|_| panic!("bip_peer: PeerManager Failed to Send SendMessage"))
                        })
                },
                |(info, mid, peer_message)| IPeerManagerMessage::SendMessage(info, mid, peer_message),
            ),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.run_with_lock_poll(|_, _, _, _, peers| {
            for peer_mut in peers.values_mut() {
                // Needs type hint in case poll fails (so that error type matches)
                let result: Poll<(), Self::SinkError> = peer_mut
                    .poll_complete()
                    .map_err(|_| panic!("bip_peer: PeerManaged Failed To Poll Peer"));

                result?;
            }

            Ok(Async::Ready(()))
        })
    }
}

//----------------------------------------------------------------------------//

/// Stream half of a `PeerManager`.
pub struct PeerManagerStream<P>
where
    P: Sink + Stream,
{
    recv: Receiver<OPeerManagerMessage<P::Item>>,
    peers: Arc<Mutex<HashMap<PeerInfo, Sender<IPeerManagerMessage<P>>>>>,
    task_queue: Arc<MsQueue<Task>>,
    opt_pending: Option<Option<OPeerManagerMessage<P::Item>>>,
}

impl<P> PeerManagerStream<P>
where
    P: Sink + Stream,
{
    fn new(
        recv: Receiver<OPeerManagerMessage<P::Item>>,
        peers: Arc<Mutex<HashMap<PeerInfo, Sender<IPeerManagerMessage<P>>>>>,
        task_queue: Arc<MsQueue<Task>>,
    ) -> PeerManagerStream<P> {
        PeerManagerStream {
            recv,
            peers,
            task_queue,
            opt_pending: None,
        }
    }

    fn run_with_lock_poll<F, T, E, I, G>(&mut self, item: I, call: F, not: G) -> Poll<T, E>
    where
        F: FnOnce(I, &mut HashMap<PeerInfo, Sender<IPeerManagerMessage<P>>>) -> Poll<T, E>,
        G: FnOnce(I) -> Option<OPeerManagerMessage<P::Item>>,
    {
        let (result, took_lock) = if let Ok(mut guard) = self.peers.try_lock() {
            let result = call(item, &mut *guard);

            // Nothing calling us will return NotReady, so we dont have to push to queue here

            (result, true)
        } else {
            // Couldnt get the lock, stash a task away
            self.task_queue.push(futures_task::current());

            // Try to get the lock once more, in case of a race condition with stashing the task
            if let Ok(mut guard) = self.peers.try_lock() {
                let result = call(item, &mut *guard);

                // Nothing calling us will return NotReady, so we dont have to push to queue here

                (result, true)
            } else {
                // If we couldnt get the lock, stash the item
                self.opt_pending = Some(not(item));

                (Ok(Async::NotReady), false)
            }
        };

        if took_lock {
            // Just notify a single person waiting on the lock to reduce contention
            if let Some(task) = self.task_queue.try_pop() {
                task.notify()
            }
        }

        result
    }
}

impl<P> Stream for PeerManagerStream<P>
where
    P: Sink + Stream,
{
    type Item = OPeerManagerMessage<P::Item>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // Intercept and propogate any messages indicating the peer shutdown so we can remove them from our peer map
        let next_message = self
            .opt_pending
            .take()
            .map(|pending| Ok(Async::Ready(pending)))
            .unwrap_or_else(|| self.recv.poll());

        next_message.and_then(|result| match result {
            Async::Ready(Some(OPeerManagerMessage::PeerRemoved(info))) => self.run_with_lock_poll(
                info,
                |info, peers| {
                    peers
                        .remove(&info)
                        .unwrap_or_else(|| panic!("bip_peer: Received PeerRemoved Message With No Matching Peer In Map"));

                    Ok(Async::Ready(Some(OPeerManagerMessage::PeerRemoved(info))))
                },
                |info| Some(OPeerManagerMessage::PeerRemoved(info)),
            ),
            Async::Ready(Some(OPeerManagerMessage::PeerDisconnect(info))) => self.run_with_lock_poll(
                info,
                |info, peers| {
                    peers
                        .remove(&info)
                        .unwrap_or_else(|| panic!("bip_peer: Received PeerDisconnect Message With No Matching Peer In Map"));

                    Ok(Async::Ready(Some(OPeerManagerMessage::PeerDisconnect(info))))
                },
                |info| Some(OPeerManagerMessage::PeerDisconnect(info)),
            ),
            Async::Ready(Some(OPeerManagerMessage::PeerError(info, error))) => self.run_with_lock_poll(
                (info, error),
                |(info, error), peers| {
                    peers
                        .remove(&info)
                        .unwrap_or_else(|| panic!("bip_peer: Received PeerError Message With No Matching Peer In Map"));

                    Ok(Async::Ready(Some(OPeerManagerMessage::PeerError(info, error))))
                },
                |(info, error)| Some(OPeerManagerMessage::PeerError(info, error)),
            ),
            other => Ok(other),
        })
    }
}

//----------------------------------------------------------------------------//

/// Trait for giving `PeerManager` message information it needs.
///
/// For any `PeerProtocol` (or plain `Codec`), that wants to be managed
/// by `PeerManager`, it must ensure that it's message type implements
/// this trait so that we have the hooks necessary to manage the peer.
pub trait ManagedMessage {
    /// Retrieve a keep alive message variant.
    fn keep_alive() -> Self;

    /// Whether or not this message is a keep alive message.
    fn is_keep_alive(&self) -> bool;
}

//----------------------------------------------------------------------------//

/// Identifier for matching sent messages with received messages.
pub type MessageId = u64;

/// Message that can be sent to the `PeerManager`.
pub enum IPeerManagerMessage<P>
where
    P: Sink,
{
    /// Add a peer to the peer manager.
    AddPeer(PeerInfo, P),
    /// Remove a peer from the peer manager.
    RemovePeer(PeerInfo),
    /// Send a message to a peer.
    SendMessage(PeerInfo, MessageId, P::SinkItem), // TODO: Support querying for statistics
}

/// Message that can be received from the `PeerManager`.
pub enum OPeerManagerMessage<M> {
    /// Message indicating a peer has been added to the peer manager.
    PeerAdded(PeerInfo),
    /// Message indicating a peer has been removed from the peer manager.
    PeerRemoved(PeerInfo),
    /// Message indicating a message has been sent to the given peer.
    SentMessage(PeerInfo, MessageId),
    /// Message indicating we have received a message from a peer.
    ReceivedMessage(PeerInfo, M),
    /// Message indicating a peer has disconnected from us.
    ///
    /// Same semantics as `PeerRemoved`, but the peer is not returned.
    PeerDisconnect(PeerInfo),
    /// Message indicating a peer errored out.
    ///
    /// Same semantics as `PeerRemoved`, but the peer is not returned.
    PeerError(PeerInfo, io::Error),
}
