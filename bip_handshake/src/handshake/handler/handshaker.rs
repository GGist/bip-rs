use std::net::SocketAddr;

use bittorrent::codec::HandshakeCodec;
use bittorrent::message::HandshakeMessage;
use message::extensions::Extensions;
use handshake::handler::HandshakeType;
use message::initiate::InitiateMessage;
use message::complete::CompleteMessage;
use filter::filters::Filters;
use handshake::handler;

use bip_util::bt::{PeerId};
use futures::future::Future;
use futures::stream::Stream;
use futures::sink::Sink;
use tokio_core::io::Io;
use tokio_core::reactor::Handle;
use tokio_timer::{self};

pub fn execute_handshake<S>(item: HandshakeType<S>, context: &(Extensions, PeerId, Filters)) -> Box<Future<Item=Option<CompleteMessage<S>>, Error=()>>
    where S: Io + 'static {
    let &(ref ext, ref pid, ref filters) = context;

    match item {
        HandshakeType::Initiate(sock, init_msg) => initiate_handshake(sock, init_msg, *ext, *pid, filters.clone()),
        HandshakeType::Complete(sock, addr)     => complete_handshake(sock, addr, *ext, *pid, filters.clone())
    }
}

fn initiate_handshake<S>(sock: S, init_msg: InitiateMessage, ext: Extensions, pid: PeerId, filters: Filters) -> Box<Future<Item=Option<CompleteMessage<S>>, Error=()>>
    where S: Io + 'static {
    let framed = sock.framed(HandshakeCodec);

    let (prot, hash, addr) = init_msg.into_parts();
    let handshake_msg = HandshakeMessage::from_parts(prot.clone(), ext, hash, pid);

    Box::new(framed.send(handshake_msg)
        .map_err(|_| ())
        .and_then(move |framed| {
            framed.into_future()
                .map_err(|_| ())
                .and_then(|(opt_msg, framed)| {
                    opt_msg.ok_or(())
                        .map(|msg| (msg, framed))
                })
                .and_then(move |(msg, framed)| {
                    let (remote_prot, remote_ext, remote_hash, remote_pid) = msg.into_parts();
                    let socket = framed.into_inner();

                    // Check that it responds with the same hash and protocol, also check our filters
                    if remote_hash != hash ||
                       remote_prot != prot ||
                       handler::should_filter(Some(&addr), Some(&remote_prot), Some(&remote_ext), Some(&remote_hash), Some(&remote_pid), &filters) {
                        Err(())
                    } else {
                        Ok(Some(CompleteMessage::new(prot, ext.union(&remote_ext), hash, remote_pid, addr, socket)))
                    }
                })
        })
        .or_else(|_| Ok(None)))
}

fn complete_handshake<S>(sock: S, addr: SocketAddr, ext: Extensions, pid: PeerId, filters: Filters) -> Box<Future<Item=Option<CompleteMessage<S>>, Error=()>>
    where S: Io + 'static {
    let framed = sock.framed(HandshakeCodec);

    Box::new(framed.into_future()
        .map_err(|_| ())
        .and_then(|(opt_msg, framed)| {
            opt_msg.ok_or(())
                .map(|msg| (msg, framed))
        })
        .and_then(move |(msg, framed)| {
            let (remote_prot, remote_ext, remote_hash, remote_pid) = msg.into_parts();
            
            // Check our filters
            if handler::should_filter(Some(&addr), Some(&remote_prot), Some(&remote_ext), Some(&remote_hash), Some(&remote_pid), &filters) {
                Err(())
            } else {
                let handshake_msg = HandshakeMessage::from_parts(remote_prot.clone(), ext, remote_hash, pid);

                Ok(framed.send(handshake_msg)
                    .map_err(|_| ())
                    .map(move |framed| {
                        let socket = framed.into_inner();

                        Some(CompleteMessage::new(remote_prot, ext.union(&remote_ext), remote_hash, remote_pid, addr, socket))
                    }))
            }
        })
        .flatten()
        .or_else(|_| Ok(None)))
}