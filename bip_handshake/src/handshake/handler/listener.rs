use std::net::SocketAddr;

use handshake::handler::HandshakeType;
use filter::filters::Filters;
use handshake::handler;

use futures::future::{self, Future};

/// Handle the result of listeneing for handshake connections.
///
/// Returns a HandshakeType that will be completed.
pub fn listener_handler<S>(item: (S, SocketAddr), context: &Filters) -> Box<Future<Item=Option<HandshakeType<S>>,Error=()>>
    where S: 'static {
    let (sock, addr) = item;

    if handler::should_filter(Some(&addr), None, None, None, None, context) {
        Box::new(future::ok(None))
    } else {
        Box::new(future::ok(Some(HandshakeType::Complete(sock, addr))))
    }
}