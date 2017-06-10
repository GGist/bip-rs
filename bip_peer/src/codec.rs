//! Codecs operating over `PeerProtocol`s.

use std::io;

use protocol::PeerProtocol;

use bytes::{BytesMut, BufMut};
use tokio_io::codec::{Decoder, Encoder};
use nom::IResult;

pub struct PeerProtocolCodec<P> {
    protocol: P
}

impl<P> PeerProtocolCodec<P> {
    pub fn new(protocol: P) -> PeerProtocolCodec<P> {
        PeerProtocolCodec{ protocol: protocol }
    }
}

impl<P> Decoder for PeerProtocolCodec<P> where P: PeerProtocol {
    type Item = P::ProtocolMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        let src_len = src.len();

        // Borrow checker...
        let mapped_result = match self.protocol.parse_bytes(src.as_ref()) {
            IResult::Done(rest, message) => IResult::Done(rest.len(), message),
            IResult::Incomplete(inc)     => IResult::Incomplete(inc),
            IResult::Error(err)          => IResult::Error(err)
        };

        match mapped_result {
            IResult::Done(rest_len, message) => {
                let consumed_len = src_len - rest_len;

                // Remove the consumed bytes
                src.split_off(consumed_len);

                Ok(Some(message))
            },
            IResult::Incomplete(_) => Ok(None),
            IResult::Error(_)      => Err(io::Error::new(io::ErrorKind::Other, "Failed To Decode Peer Protocol"))
        }
    }
}

impl<P> Encoder for PeerProtocolCodec<P> where P: PeerProtocol {
    type Item = P::ProtocolMessage;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> io::Result<()> {
        self.protocol.write_bytes(&item, dst.writer())
    }
}