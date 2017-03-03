extern crate bip_handshake;
extern crate tokio_core;
extern crate futures;

use std::any::{Any, TypeId};
use std::net::SocketAddr;
use std::hash::Hash;
use std::cmp::{PartialEq, Eq};
use std::fmt::Debug;
use std::collections::hash_map::DefaultHasher;

use futures::future::{self, Future, Loop};
use futures::stream::Stream;
use futures::sink::Sink;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;

use bip_handshake::{HandshakerBuilder, InitiateHandshake};

fn main() {
    let mut core = Core::new().unwrap();
    let handshaker = HandshakerBuilder::new().build::<TcpStream>(core.handle()).unwrap();
    let (sink, stream) = handshaker.split();

    println!("{:?}", sink.port());

    let hash = [0x66, 0xa9, 0x2e, 0xc7, 0x7b, 0x81, 0xc3, 0xdf, 0x07, 0xe0, 0x07, 0x81, 0xfd, 0x3b, 0xdf, 0x65, 0x4b, 0x6e, 0xf1, 0xa2];
    let initiate = InitiateHandshake::new("BitTorrent protocol".to_string(), hash.into(), "127.0.0.1:49658".parse().unwrap());

    let asd = sink.send(initiate).wait().unwrap();

    core.run(future::loop_fn(stream, |stream| {
        stream.into_future()
            .and_then(|(opt, stream)| {
                println!("Connected!!!");
                let result: Loop<(), _> = Loop::Continue(stream);
                loop {
                    
                }
                Ok(result)
            })
            .or_else(|(err, stream)| {
                println!("Errored!!!");

                Err(())
            })
    }));
}