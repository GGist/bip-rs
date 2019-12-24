use std::io::{self, Cursor};

use crate::bittorrent::message::{self, HandshakeMessage};

use bytes::buf::BufMut;
use bytes::BytesMut;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::{Async, AsyncSink, Poll, StartSend};
use tokio_io::try_nb;
use tokio_io::{AsyncRead, AsyncWrite};

enum HandshakeState {
    Waiting,
    Length(u8),
    Finished,
}

// We can't use the built in frames because they may buffer more
// bytes than we need for a handshake. That is unacceptable for us
// because we are giving a raw socket to the client of this library.
// We don't want to steal any of their bytes during our handshake!
pub struct FramedHandshake<S> {
    sock: S,
    write_buffer: BytesMut,
    read_buffer: Vec<u8>,
    read_pos: usize,
    state: HandshakeState,
}

impl<S> FramedHandshake<S> {
    pub fn new(sock: S) -> FramedHandshake<S> {
        FramedHandshake {
            sock,
            write_buffer: BytesMut::with_capacity(1),
            read_buffer: vec![0],
            read_pos: 0,
            state: HandshakeState::Waiting,
        }
    }

    pub fn into_inner(self) -> S {
        self.sock
    }
}

impl<S> Sink for FramedHandshake<S>
where
    S: AsyncWrite,
{
    type SinkItem = HandshakeMessage;
    type SinkError = io::Error;

    fn start_send(&mut self, item: HandshakeMessage) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.write_buffer.reserve(item.write_len());
        item.write_bytes(self.write_buffer.by_ref().writer())?;

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        loop {
            let write_result = self.sock.write_buf(&mut Cursor::new(&self.write_buffer));

            match try_nb!(write_result) {
                Async::Ready(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "Failed To Write Bytes")),
                Async::Ready(written) => {
                    self.write_buffer.split_to(written);
                }
                Async::NotReady => return Ok(Async::NotReady),
            }

            if self.write_buffer.is_empty() {
                try_nb!(self.sock.flush());

                return Ok(Async::Ready(()));
            }
        }
    }
}

impl<S> Stream for FramedHandshake<S>
where
    S: AsyncRead,
{
    type Item = HandshakeMessage;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match self.state {
                HandshakeState::Waiting => {
                    let read_result = self.sock.read_buf(&mut Cursor::new(&mut self.read_buffer[..]));

                    match try_nb!(read_result) {
                        Async::Ready(0) => return Ok(Async::Ready(None)),
                        Async::Ready(1) => {
                            let length = self.read_buffer[0];

                            self.state = HandshakeState::Length(length);

                            self.read_pos = 1;
                            self.read_buffer = vec![0u8; message::write_len_with_protocol_len(length)];
                            self.read_buffer[0] = length;
                        }
                        Async::Ready(read) => panic!("bip_handshake: Expected To Read Single Byte, Read {:?}", read),
                        Async::NotReady => return Ok(Async::NotReady),
                    }
                }
                HandshakeState::Length(length) => {
                    let expected_length = message::write_len_with_protocol_len(length);

                    if self.read_pos == expected_length {
                        match HandshakeMessage::from_bytes(&*self.read_buffer) {
                            Ok((_, message)) => {
                                self.state = HandshakeState::Finished;

                                return Ok(Async::Ready(Some(message)));
                            }
                            Err(nom::Err::Incomplete(_)) => panic!("bip_handshake: HandshakeMessage Failed With Incomplete Bytes"),
                            Err(_) => return Err(io::Error::new(io::ErrorKind::InvalidData, "HandshakeMessage Failed To Parse")),
                        }
                    } else {
                        let read_result = {
                            let mut cursor = Cursor::new(&mut self.read_buffer[self.read_pos..]);

                            try_nb!(self.sock.read_buf(&mut cursor))
                        };

                        match read_result {
                            Async::Ready(0) => return Ok(Async::Ready(None)),
                            Async::Ready(read) => {
                                self.read_pos += read;
                            }
                            Async::NotReady => return Ok(Async::NotReady),
                        }
                    }
                }
                HandshakeState::Finished => return Ok(Async::Ready(None)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::FramedHandshake;
    use crate::bittorrent::message::HandshakeMessage;
    use crate::message::extensions::{self, Extensions};
    use crate::message::protocol::Protocol;

    use bip_util::bt::{self, InfoHash, PeerId};
    use futures::sink::Sink;
    use futures::stream::Stream;
    use futures::Future;

    fn any_peer_id() -> PeerId {
        [22u8; bt::PEER_ID_LEN].into()
    }

    fn any_info_hash() -> InfoHash {
        [55u8; bt::INFO_HASH_LEN].into()
    }

    fn any_extensions() -> Extensions {
        [255u8; extensions::NUM_EXTENSION_BYTES].into()
    }

    #[test]
    fn positive_write_handshake_message() {
        let message = HandshakeMessage::from_parts(Protocol::BitTorrent, any_extensions(), any_info_hash(), any_peer_id());

        let write_frame = FramedHandshake::new(Cursor::new(Vec::new())).send(message.clone()).wait().unwrap();
        let recv_buffer = write_frame.into_inner().into_inner();

        let mut exp_buffer = Vec::new();
        message.write_bytes(&mut exp_buffer).unwrap();

        assert_eq!(exp_buffer, recv_buffer);
    }

    #[test]
    fn positive_write_multiple_handshake_messages() {
        let message_one = HandshakeMessage::from_parts(Protocol::BitTorrent, any_extensions(), any_info_hash(), any_peer_id());
        let message_two = HandshakeMessage::from_parts(Protocol::Custom(vec![5, 6, 7]), any_extensions(), any_info_hash(), any_peer_id());

        let write_frame = FramedHandshake::new(Cursor::new(Vec::new()))
            .send(message_one.clone())
            .wait()
            .unwrap()
            .send(message_two.clone())
            .wait()
            .unwrap();
        let recv_buffer = write_frame.into_inner().into_inner();

        let mut exp_buffer = Vec::new();
        message_one.write_bytes(&mut exp_buffer).unwrap();
        message_two.write_bytes(&mut exp_buffer).unwrap();

        assert_eq!(exp_buffer, recv_buffer);
    }

    #[test]
    fn positive_read_handshake_message() {
        let exp_message = HandshakeMessage::from_parts(Protocol::BitTorrent, any_extensions(), any_info_hash(), any_peer_id());

        let mut buffer = Vec::new();
        exp_message.write_bytes(&mut buffer).unwrap();

        let mut read_iter = FramedHandshake::new(&buffer[..]).wait();
        let recv_message = read_iter.next().unwrap().unwrap();
        assert!(read_iter.next().is_none());

        assert_eq!(exp_message, recv_message);
    }

    #[test]
    fn positive_read_byte_after_handshake() {
        let exp_message = HandshakeMessage::from_parts(Protocol::BitTorrent, any_extensions(), any_info_hash(), any_peer_id());

        let mut buffer = Vec::new();
        exp_message.write_bytes(&mut buffer).unwrap();
        // Write some bytes right after the handshake, make sure
        // our framed handshake doesnt read/buffer these (we need
        // to be able to read them afterwards)
        buffer.write_all(&[55]).unwrap();

        let read_frame = FramedHandshake::new(&buffer[..]).into_future().wait().ok().unwrap().1;
        let buffer_ref = read_frame.into_inner();

        assert_eq!(&[55], buffer_ref);
    }

    #[test]
    fn positive_read_bytes_after_handshake() {
        let exp_message = HandshakeMessage::from_parts(Protocol::BitTorrent, any_extensions(), any_info_hash(), any_peer_id());

        let mut buffer = Vec::new();
        exp_message.write_bytes(&mut buffer).unwrap();
        // Write some bytes right after the handshake, make sure
        // our framed handshake doesnt read/buffer these (we need
        // to be able to read them afterwards)
        buffer.write_all(&[55, 54, 21]).unwrap();

        let read_frame = FramedHandshake::new(&buffer[..]).into_future().wait().ok().unwrap().1;
        let buffer_ref = read_frame.into_inner();

        assert_eq!(&[55, 54, 21], buffer_ref);
    }
}
