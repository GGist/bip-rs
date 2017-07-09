//! Codecs operating over `PeerProtocol`s.

use std::io;

use protocol::PeerProtocol;

use bytes::{BytesMut, BufMut};
use tokio_io::codec::{Decoder, Encoder};

/// Codec operating over some `PeerProtocol`.
pub struct PeerProtocolCodec<P> {
    protocol:    P,
    max_payload: Option<usize>
}

impl<P> PeerProtocolCodec<P> {
    /// Create a new `PeerProtocolCodec`.
    ///
    /// It is strongly recommended to use `PeerProtocolCodec::with_max_payload`
    /// instead of this function, as this function will not enforce a limit on
    /// received payload length.
    pub fn new(protocol: P) -> PeerProtocolCodec<P> {
        PeerProtocolCodec{ protocol: protocol, max_payload: None }
    }

    /// Create a new `PeerProtocolCodec` which will yield an error if 
    /// receiving a payload larger than the specified `max_payload`.
    pub fn with_max_payload(protocol: P, max_payload: usize) -> PeerProtocolCodec<P> {
        PeerProtocolCodec{ protocol: protocol, max_payload: Some(max_payload) }
    }
}

impl<P> Decoder for PeerProtocolCodec<P> where P: PeerProtocol {
    type Item = P::ProtocolMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        let src_len = src.len();
        
        let bytes = match try!(self.protocol.bytes_needed(src.as_ref())) {
            Some(needed) if self.max_payload
                                .map(|max_payload| needed > max_payload)
                                .unwrap_or(false) => {
                return Err(io::Error::new(io::ErrorKind::Other, "PeerProtocolCodec Enforced Maximum Payload Check For Peer"))
            }
            Some(needed) if needed <= src_len => src.split_to(needed).freeze(),
            Some(_) | None                    => { return Ok(None) }
        };

        self.protocol.parse_bytes(bytes).map(|message| Some(message))
    }
}

impl<P> Encoder for PeerProtocolCodec<P> where P: PeerProtocol {
    type Item = P::ProtocolMessage;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> io::Result<()> {
        dst.reserve(self.protocol.message_size(&item));
        
        self.protocol.write_bytes(&item, dst.writer())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::PeerProtocolCodec;
    use protocol::PeerProtocol;

    use bytes::{Bytes, BytesMut};
    use tokio_io::codec::{Decoder};

    struct ConsumeProtocol;

    impl PeerProtocol for ConsumeProtocol {
        type ProtocolMessage = ();

        fn bytes_needed(&mut self, bytes: &[u8]) -> io::Result<Option<usize>> {
            Ok(Some(bytes.len()))
        }

        fn parse_bytes(&mut self, _bytes: Bytes) -> io::Result<Self::ProtocolMessage> {
            Ok(())
        }

        fn write_bytes<W>(&mut self, _message: &Self::ProtocolMessage, _writer: W) -> io::Result<()>
            where W: Write {
            Ok(())
        }

        fn message_size(&mut self, _message: &Self::ProtocolMessage) -> usize {
            0
        }
    }

    #[test]
    fn positive_parse_at_max_payload() {
        let mut codec = PeerProtocolCodec::with_max_payload(ConsumeProtocol, 100);
        let mut bytes = BytesMut::with_capacity(100);

        bytes.extend_from_slice(&[0u8; 100]);

        assert_eq!(Some(()), codec.decode(&mut bytes).unwrap());
        assert_eq!(bytes.len(), 0);
    }

    #[test]
    fn negative_parse_above_max_payload() {
        let mut codec = PeerProtocolCodec::with_max_payload(ConsumeProtocol, 100);
        let mut bytes = BytesMut::with_capacity(200);

        bytes.extend_from_slice(&[0u8; 200]);

        assert!(codec.decode(&mut bytes).is_err());
        assert_eq!(bytes.len(), 200);
    }
}