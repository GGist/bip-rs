use std::net::SocketAddr;

use crate::filter::filters::Filters;
use crate::filter::FilterDecision;
use crate::message::extensions::Extensions;
use crate::message::initiate::InitiateMessage;
use crate::message::protocol::Protocol;

use bip_util::bt::{InfoHash, PeerId};
use futures::future::{self, Future, IntoFuture, Loop};
use futures::sink::Sink;
use futures::stream::Stream;
use tokio_core::reactor::Handle;

pub mod handshaker;
pub mod initiator;
pub mod listener;
pub mod timer;

pub enum HandshakeType<S> {
    Initiate(S, InitiateMessage),
    Complete(S, SocketAddr),
}

enum LoopError<D> {
    Terminate,
    Recoverable(D),
}

/// Create loop for feeding the handler with the items coming from the stream,
/// and forwarding the result to the sink.
///
/// If the stream is used up, or an error is propogated from any of the
/// elements, the loop will terminate.
pub fn loop_handler<M, H, K, F, R, C>(stream: M, handler: H, sink: K, context: C, handle: &Handle)
where
    M: Stream + 'static,
    H: FnMut(M::Item, &C) -> F + 'static,
    K: Sink<SinkItem = R> + 'static,
    F: IntoFuture<Item = Option<R>> + 'static,
    R: 'static,
    C: 'static,
{
    handle.spawn(future::loop_fn(
        (stream, handler, sink, context),
        |(stream, mut handler, sink, context)| {
            // We will terminate the loop if, the stream gives us an error, the stream gives
            // us None, the handler gives us an error, or the sink gives us an
            // error. If the handler gives us Ok(None), we will map that to a
            // recoverable error (since our Ok(Some) result would have to continue with its
            // own future, we hijack the error to store an immediate value). We
            // finally map any recoverable errors back to an Ok value
            // so we can continue with the loop in that case.
            stream
                .into_future()
                .map_err(|_| LoopError::Terminate)
                .and_then(|(opt_item, stream)| {
                    opt_item
                        .ok_or(LoopError::Terminate)
                        .map(|item| (item, stream))
                })
                .and_then(move |(item, stream)| {
                    let into_future = handler(item, &context);

                    into_future
                        .into_future()
                        .map_err(|_| LoopError::Terminate)
                        .and_then(move |opt_result| match opt_result {
                            Some(result) => Ok((result, stream, handler, context, sink)),
                            None => Err(LoopError::Recoverable((stream, handler, context, sink))),
                        })
                })
                .and_then(|(result, stream, handler, context, sink)| {
                    sink.send(result)
                        .map_err(|_| LoopError::Terminate)
                        .map(|sink| Loop::Continue((stream, handler, sink, context)))
                })
                .or_else(|loop_error| match loop_error {
                    LoopError::Terminate => Err(()),
                    LoopError::Recoverable((stream, handler, context, sink)) => {
                        Ok(Loop::Continue((stream, handler, sink, context)))
                    }
                })
        },
    ));
}

/// Computes whether or not we should filter given the parameters and filters.
pub fn should_filter(
    addr: Option<&SocketAddr>,
    prot: Option<&Protocol>,
    ext: Option<&Extensions>,
    hash: Option<&InfoHash>,
    pid: Option<&PeerId>,
    filters: &Filters,
) -> bool {
    // Initially, we set all our results to pass
    let mut addr_filter = FilterDecision::Pass;
    let mut prot_filter = FilterDecision::Pass;
    let mut ext_filter = FilterDecision::Pass;
    let mut hash_filter = FilterDecision::Pass;
    let mut pid_filter = FilterDecision::Pass;

    // Choose on individual fields
    filters.access_filters(|ref_filters| {
        for ref_filter in ref_filters {
            addr_filter = addr_filter.choose(ref_filter.on_addr(addr));
            prot_filter = prot_filter.choose(ref_filter.on_prot(prot));
            ext_filter = ext_filter.choose(ref_filter.on_ext(ext));
            hash_filter = hash_filter.choose(ref_filter.on_hash(hash));
            pid_filter = pid_filter.choose(ref_filter.on_pid(pid));
        }
    });

    // Choose across the results of individual fields
    addr_filter
        .choose(prot_filter)
        .choose(ext_filter)
        .choose(hash_filter)
        .choose(pid_filter)
        == FilterDecision::Block
}
