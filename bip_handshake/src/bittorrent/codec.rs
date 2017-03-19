use std::io;

use bittorrent::message::HandshakeMessage;

use bytes::BytesMut;
use bytes::buf::BufMut;
use nom::{IResult};
use tokio_io::codec::{Encoder, Decoder};

pub struct HandshakeCodec;

impl Decoder for HandshakeCodec {
    type Item = HandshakeMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<HandshakeMessage>> {
        match HandshakeMessage::from_bytes(&**src) {
            IResult::Done(_, message) => Ok(Some(message)),
            IResult::Incomplete(_)    => Ok(None),
            IResult::Error(_)         => Err(io::Error::new(io::ErrorKind::ConnectionAborted, "Handshake Protocol Error"))
        }
    }
}

impl Encoder for HandshakeCodec {
    type Item = HandshakeMessage;
    type Error = io::Error;

    fn encode(&mut self, msg: HandshakeMessage, dst: &mut BytesMut) -> io::Result<()> {
        msg.write_bytes(dst.writer())
    }
}