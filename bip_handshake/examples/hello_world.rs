/*extern crate tokio_core;
extern crate tokio_io;
extern crate bytes;
extern crate futures;

use std::thread;
use std::io;

use bytes::*;
use tokio_io::AsyncRead;
use tokio_io::codec::*;
use tokio_core::*;
use tokio_core::reactor::*;
use tokio_core::net::*;
use futures::stream::*;
use futures::*;

struct SimpleProtocol;

impl Decoder for SimpleProtocol {
    type Item = u8;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<u8>> {
        if src.len() >= 1 {
            Ok(Some(src.split_off(1)[0]))
        } else {
            Ok(None)
        }
    }
}

impl Encoder for SimpleProtocol {
    type Item = ();
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        unimplemented!()
    }
}

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&("127.0.0.1:32423".parse().unwrap()), &handle).unwrap();

    handle.spawn(listener.incoming()
        .into_future()
        .map_err(|_| ())
        .and_then(|(opt_result, _)| {
            let (socket, addr) = opt_result.unwrap();

            socket.framed(SimpleProtocol)
                .into_future()
                .and_then(|(opt_message, framed)| {
                    println!("{:?}", opt_message);

                    framed.into_inner().framed(SimpleProtocol)
                        .into_future()
                        .and_then(|(opt_message, _)| {
                            println!("{:?}", opt_message);

                            Err(())
                        })
                })
        })
        .map(|_| ())
        .map_err(|_| ())
    );
}*/