use std::io::{self, Cursor};

use bittorrent::message::{self, HandshakeMessage};

use bytes::BytesMut;
use futures::{StartSend, AsyncSink, Async, Poll};
use futures::sink::Sink;
use futures::stream::Stream;
use tokio_io::{AsyncWrite, AsyncRead};
use nom::IResult;

enum HandshakeState {
    Waiting,
    Length(u8),
    Finished
}

// We can't use the built in frames because they may buffer more
// bytes than we need for a handshake. That is unacceptable for us
// because we are giving a raw socket to the client of this library.
// We don't want to steal any of their bytes during our handshake!
pub struct FramedHandshake<S> {
    sock:         S,
    write_buffer: BytesMut,
    read_buffer:  BytesMut,
    state:        HandshakeState
}

impl<S> FramedHandshake<S> {
    pub fn new(sock: S) -> FramedHandshake<S> {
        FramedHandshake{ sock: sock, write_buffer: BytesMut::with_capacity(1),
                         read_buffer: BytesMut::with_capacity(1), state: HandshakeState::Waiting }
    }

    pub fn into_inner(self) -> S {
        self.sock
    }
}

impl<S> Sink for FramedHandshake<S> where S: AsyncWrite {
    type SinkItem = HandshakeMessage;
    type SinkError = io::Error;

    fn start_send(&mut self, item: HandshakeMessage) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.write_buffer.reserve(item.write_len());
        try!(item.write_bytes(&mut *self.write_buffer));

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        loop {
            let write_result = {
                self.sock.write_buf(&mut Cursor::new(&self.write_buffer))
            };

            match try!(write_result) {
                Async::Ready(written) => { self.write_buffer.split_to(written); },
                Async::NotReady       => { return Ok(Async::NotReady) }
            }

            if self.write_buffer.is_empty() {
                return Ok(Async::Ready(()))
            }
        }
    }
}

impl<S> Stream for FramedHandshake<S> where S: AsyncRead {
    type Item = HandshakeMessage;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match self.state {
                HandshakeState::Waiting => {
                    self.read_buffer.reserve(1);

                    let read_result = try!(self.sock.read_buf(&mut self.read_buffer));
                    match read_result {
                        Async::Ready(0)    => (),
                        Async::Ready(1)    => { self.state = HandshakeState::Length(self.read_buffer.split_off(1)[0]); },
                        Async::Ready(read) => panic!("bip_handshake: Expected To Read Single Byte, Read {:?}", read),
                        Async::NotReady    => { return Ok(Async::NotReady) }
                    }
                },
                HandshakeState::Length(length) => {
                    let expected_length = message::write_len_with_protocol_len(length);

                    if self.read_buffer.len() == expected_length {
                        match HandshakeMessage::from_bytes(&*self.read_buffer) {
                            IResult::Done(_, message) => { self.state = HandshakeState::Finished; return Ok(Async::Ready(Some(message))) },
                            IResult::Incomplete(_)    => panic!("bip_handshake: HandshakeMessage Failed With Incomplete Bytes"),
                            IResult::Error(_)         => { return Err(io::Error::new(io::ErrorKind::InvalidData, "HandshakeMessage Failed To Parse")) }
                        }
                    } else {
                        self.read_buffer.reserve(length as usize);

                        let read_result = try!(self.sock.read_buf(&mut self.read_buffer));
                        match read_result {
                            Async::Ready(_)    => (),
                            Async::NotReady    => { return Ok(Async::NotReady) }
                        }
                    }
                },
                HandshakeState::Finished => { return Ok(Async::Ready(None)) }
            }
        }
    }
}