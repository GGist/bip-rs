#![allow(deprecated)]

use std::io;

use crate::manager::builder::PeerManagerBuilder;
use crate::manager::future::{
    PersistentError, PersistentStream, RecurringTimeoutError, RecurringTimeoutStream,
};
use crate::manager::peer_info::PeerInfo;
use crate::manager::{IPeerManagerMessage, ManagedMessage, OPeerManagerMessage};

use futures::future::{self, Future, Loop};
use futures::sink::Sink;
use futures::stream::{MergedItem, Stream};
use futures::sync::mpsc::{self, Sender};
use tokio_core::reactor::Handle;
use tokio_timer::Timer;

// Separated from MergedError to
enum PeerError {
    // We need to send a heartbeat (no messages sent from manager for a while)
    ManagerHeartbeatInterval,
    // Manager error (or expected shutdown)
    ManagerDisconnect,
    // Peer errors
    PeerDisconnect,
    PeerError(io::Error),
    PeerNoHeartbeat,
}

enum MergedError<A, B, C> {
    Peer(PeerError),
    // Fake error types (used to stash future "futures" into an error type to be
    // executed in a different future transformation, so we dont have to box them)
    StageOne(A),
    StageTwo(B),
    StageThree(C),
}

//----------------------------------------------------------------------------//

pub fn run_peer<P>(
    peer: P,
    info: PeerInfo,
    o_send: Sender<OPeerManagerMessage<P::Item>>,
    timer: Timer,
    builder: &PeerManagerBuilder,
    handle: &Handle,
) -> Sender<IPeerManagerMessage<P>>
where
    P: Stream<Error = io::Error> + Sink<SinkError = io::Error> + 'static,
    P::SinkItem: ManagedMessage,
    P::Item: ManagedMessage,
{
    let (m_send, m_recv) = mpsc::channel(builder.sink_buffer_capacity());
    let (p_send, p_recv) = peer.split();

    // Build a stream that will timeout if no message is sent for heartbeat_timeout
    // and teardown (dont preserve) the underlying stream
    let p_stream = timer
        .timeout_stream(PersistentStream::new(p_recv), builder.heartbeat_timeout())
        .map_err(|error| match error {
            PersistentError::Disconnect => PeerError::PeerDisconnect,
            PersistentError::Timeout => PeerError::PeerNoHeartbeat,
            PersistentError::IoError(err) => PeerError::PeerError(err),
        });
    // Build a stream that will notify us of no message is sent for
    // heartbeat_interval and done teartdown (preserve) the underlying stream
    let m_stream = RecurringTimeoutStream::new(m_recv, timer, builder.heartbeat_interval())
        .map_err(|error| match error {
            RecurringTimeoutError::Disconnect => PeerError::ManagerDisconnect,
            RecurringTimeoutError::Timeout => PeerError::ManagerHeartbeatInterval,
        });

    let merged_stream = m_stream.merge(p_stream);

    handle.spawn(
        o_send
            .send(OPeerManagerMessage::PeerAdded(info))
            .map_err(|_| ())
            .and_then(move |o_send| {
                future::loop_fn((merged_stream, o_send, p_send, info), |(merged_stream, o_send, p_send, info)| {
                    // Our return tuple takes the form (merged_stream, Option<Send Message>, Option<Recv Message>, Option<Send To Manager Message>, is_good) where each stage (A, B, C),
                    // will execute one of those options (if present), since each future transform can only execute a single future and we have 2^3 possible combintations
                    // (Some or None = 2)^(3 Options = 3)
                    merged_stream
                        .into_future()
                        .then(move |result| {
                            let result = match result {
                                Ok((Some(MergedItem::First(IPeerManagerMessage::SendMessage(p_info, mid, p_message))), merged_stream)) => {
                                    Ok((merged_stream, Some(p_message), None, Some(OPeerManagerMessage::SentMessage(p_info, mid)), true))
                                }
                                Ok((Some(MergedItem::First(IPeerManagerMessage::RemovePeer(p_info))), merged_stream)) => {
                                    Ok((merged_stream, None, None, Some(OPeerManagerMessage::PeerRemoved(p_info)), false))
                                }
                                Ok((Some(MergedItem::Second(peer_message)), merged_stream)) => {
                                    Ok((merged_stream, None, Some(peer_message), None, true))
                                }
                                Ok((
                                    Some(MergedItem::Both(IPeerManagerMessage::SendMessage(p_info, mid, p_message), peer_message)),
                                    merged_stream,
                                )) => Ok((
                                    merged_stream,
                                    Some(p_message),
                                    Some(peer_message),
                                    Some(OPeerManagerMessage::SentMessage(p_info, mid)),
                                    true,
                                )),
                                Ok((Some(MergedItem::Both(IPeerManagerMessage::RemovePeer(p_info), peer_message)), merged_stream)) => {
                                    Ok((merged_stream, None, Some(peer_message), Some(OPeerManagerMessage::PeerRemoved(p_info)), false))
                                }
                                Ok((Some(_), _)) => panic!("bip_peer: Peer Future Received Invalid Message From Peer Manager"),
                                Err((PeerError::ManagerHeartbeatInterval, merged_stream)) => {
                                    Ok((merged_stream, Some(P::SinkItem::keep_alive()), None, None, true))
                                }
                                // In this case, the manager and peer probably both disconnected at the same time? Treat as a manager disconnect.
                                Ok((None, _)) => Err(MergedError::Peer(PeerError::ManagerDisconnect)),
                                Err((PeerError::ManagerDisconnect, _)) => Err(MergedError::Peer(PeerError::ManagerDisconnect)),
                                Err((PeerError::PeerDisconnect, merged_stream)) => {
                                    Ok((merged_stream, None, None, Some(OPeerManagerMessage::PeerDisconnect(info)), false))
                                }
                                Err((PeerError::PeerError(err), merged_stream)) => {
                                    Ok((merged_stream, None, None, Some(OPeerManagerMessage::PeerError(info, err)), false))
                                }
                                Err((PeerError::PeerNoHeartbeat, merged_stream)) => {
                                    Ok((merged_stream, None, None, Some(OPeerManagerMessage::PeerDisconnect(info)), false))
                                }
                            };

                            match result {
                                Ok((merged_stream, opt_send, opt_recv, opt_ack, is_good)) => {
                                    if let Some(send) = opt_send {
                                        Ok(p_send
                                            .send(send)
                                            .map_err(|_| MergedError::Peer(PeerError::PeerDisconnect))
                                            .and_then(move |p_send| {
                                                Err(MergedError::StageOne((
                                                    merged_stream,
                                                    o_send,
                                                    p_send,
                                                    info,
                                                    opt_recv,
                                                    opt_ack,
                                                    is_good,
                                                )))
                                            }))
                                    } else {
                                        Err(MergedError::StageOne((merged_stream, o_send, p_send, info, opt_recv, opt_ack, is_good)))
                                    }
                                }
                                Err(err) => Err(err),
                            }
                        })
                        .flatten()
                        .or_else(|error| {
                            match error {
                                MergedError::StageOne((merged_stream, o_send, p_send, info, opt_recv, opt_ack, is_good)) => {
                                    if let Some(recv) = opt_recv {
                                        if !recv.is_keep_alive() {
                                            return Ok(o_send
                                                .send(OPeerManagerMessage::ReceivedMessage(info, recv))
                                                .map_err(|_| MergedError::Peer(PeerError::ManagerDisconnect))
                                                .and_then(move |o_send| {
                                                    Err(MergedError::StageTwo((merged_stream, o_send, p_send, info, opt_ack, is_good)))
                                                }));
                                        }
                                    }

                                    // Either we had no recv message (from remote), or it was a keep alive message, which we dont propagate
                                    Err(MergedError::StageTwo((merged_stream, o_send, p_send, info, opt_ack, is_good)))
                                }
                                err => Err(err),
                            }
                        })
                        .flatten()
                        .or_else(|error| match error {
                            MergedError::StageTwo((merged_stream, o_send, p_send, info, opt_ack, is_good)) => {
                                if let Some(ack) = opt_ack {
                                    Ok(o_send
                                        .send(ack)
                                        .map_err(|_| MergedError::Peer(PeerError::ManagerDisconnect))
                                        .and_then(move |o_send| {
                                            Err(MergedError::StageThree((merged_stream, o_send, p_send, info, is_good)))
                                        }))
                                } else {
                                    Err(MergedError::StageThree((merged_stream, o_send, p_send, info, is_good)))
                                }
                            }
                            err => Err(err),
                        })
                        .flatten()
                        .or_else(|error| {
                            match error {
                                MergedError::StageThree((merged_stream, o_send, p_send, info, is_good)) => {
                                    // Connection is good if no errors occurred (we do this so we can use the same plumbing)
                                    // for sending "acks" back to our manager when an error occurrs, we just have None, None,
                                    // Some, false when we want to send an error message to the manager, but terminate the connection.
                                    if is_good {
                                        Ok(Loop::Continue((merged_stream, o_send, p_send, info)))
                                    } else {
                                        Ok(Loop::Break(()))
                                    }
                                }
                                _ => Ok(Loop::Break(())),
                            }
                        })
                })
            }),
    );

    m_send
}
