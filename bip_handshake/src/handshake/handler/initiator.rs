use handshake::handler::HandshakeType;
use transport::Transport;
use message::initiate::InitiateMessage;
use filter::filters::Filters;
use handshake::handler;

use futures::future::{self, Future};
use tokio_core::reactor::Handle;

/// Handle the initiation of connections, which are returned as a HandshakeType.
pub fn initiator_handler<T>(item: InitiateMessage, context: &(Filters, Handle)) -> Box<Future<Item=Option<HandshakeType<T::Socket>>,Error=()>>
    where T: Transport {
    let &(ref filters, ref handle) = context;

    if handler::should_filter(Some(item.address()), Some(item.protocol()), None, Some(item.hash()), None, filters) {
        Box::new(future::ok(None))
    } else {
        let res_connect = T::connect(item.address(), handle);

        Box::new(future::lazy(|| res_connect)
            .flatten()
            .map_err(|_| ())
            .map(|socket| {
                Some(HandshakeType::Initiate(socket, item))
            }))
    }
}