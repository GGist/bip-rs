use std::io;

use bittorrent::handshake::message::HandshakeMessage;

use nom::{IResult};
use tokio_core::io::{Codec, EasyBuf};

pub struct HandshakeCodec;

impl Codec for HandshakeCodec {
    type In = HandshakeMessage;
    type Out = HandshakeMessage;

    fn decode(&mut self, buf: &mut EasyBuf) -> io::Result<Option<HandshakeMessage>> {
        match HandshakeMessage::from_bytes(buf.as_ref()) {
            IResult::Done(_, message) => Ok(Some(message)),
            IResult::Incomplete(_)    => Ok(None),
            IResult::Error(_)         => Err(io::Error::new(io::ErrorKind::ConnectionAborted, "Handshake Protocol Error"))
        }
    }

    fn encode(&mut self, msg: HandshakeMessage, buf: &mut Vec<u8>) -> io::Result<()> {
        msg.write_bytes(buf)
    }
}