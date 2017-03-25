use std::net::SocketAddr;

use handshake::handler::HandshakeType;
use filter::filters::Filters;
use handshake::handler;

use futures::{Poll, Async};
use futures::future::{Future};

pub struct ListenerHandler<S> {
    opt_item: Option<HandshakeType<S>>
}

impl<S> ListenerHandler<S> {
    pub fn new(item: (S, SocketAddr), context: &Filters) -> ListenerHandler<S> {
        let (sock, addr) = item;

        let opt_item = if handler::should_filter(Some(&addr), None, None, None, None, context) {
            None
        } else {
            Some(HandshakeType::Complete(sock, addr))
        };

        ListenerHandler{ opt_item: opt_item }
    }
}

impl<S> Future for ListenerHandler<S> {
    type Item = Option<HandshakeType<S>>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<HandshakeType<S>>, ()> {
        Ok(Async::Ready(self.opt_item.take()))
    }
}