use std::io;
use std::io::Write;
use std::u8;

use crate::message::extensions::{self, Extensions};
use crate::message::protocol::Protocol;

use bip_util::bt::{self, InfoHash, PeerId};
use nom::{call, do_parse, take, IResult};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HandshakeMessage {
    prot: Protocol,
    ext: Extensions,
    hash: InfoHash,
    pid: PeerId,
}

impl HandshakeMessage {
    /// Create a new `HandshakeMessage` from the given components.
    pub fn from_parts(
        prot: Protocol,
        ext: Extensions,
        hash: InfoHash,
        pid: PeerId,
    ) -> HandshakeMessage {
        if let Protocol::Custom(ref custom) = prot {
            if custom.len() > u8::max_value() as usize {
                panic!(
                    "bip_handshake: Handshake Message With Protocol Length Greater Than {} Found",
                    u8::max_value()
                )
            }
        }

        HandshakeMessage {
            prot,
            ext,
            hash,
            pid,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], HandshakeMessage> {
        parse_remote_handshake(bytes)
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
    where
        W: Write,
    {
        self.prot.write_bytes(&mut writer)?;
        self.ext.write_bytes(&mut writer)?;
        writer.write_all(self.hash.as_ref())?;
        writer.write_all(self.pid.as_ref())?;

        Ok(())
    }

    pub fn write_len(&self) -> usize {
        write_len_with_protocol_len(self.prot.write_len() as u8)
    }

    pub fn into_parts(self) -> (Protocol, Extensions, InfoHash, PeerId) {
        (self.prot, self.ext, self.hash, self.pid)
    }
}

pub fn write_len_with_protocol_len(protocol_len: u8) -> usize {
    1 + (protocol_len as usize)
        + extensions::NUM_EXTENSION_BYTES
        + bt::INFO_HASH_LEN
        + bt::PEER_ID_LEN
}

fn parse_remote_handshake(bytes: &[u8]) -> IResult<&[u8], HandshakeMessage> {
    do_parse!(
        bytes,
        prot: call!(Protocol::from_bytes)
            >> ext: call!(Extensions::from_bytes)
            >> hash: call!(parse_remote_hash)
            >> pid: call!(parse_remote_pid)
            >> (HandshakeMessage::from_parts(prot, ext, hash, pid))
    )
}

fn parse_remote_hash(bytes: &[u8]) -> IResult<&[u8], InfoHash> {
    do_parse!(
        bytes,
        hash: take!(bt::INFO_HASH_LEN) >> (InfoHash::from_hash(hash).unwrap())
    )
}

fn parse_remote_pid(bytes: &[u8]) -> IResult<&[u8], PeerId> {
    do_parse!(
        bytes,
        pid: take!(bt::PEER_ID_LEN) >> (PeerId::from_hash(pid).unwrap())
    )
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::HandshakeMessage;
    use crate::message::extensions::{self, Extensions};
    use crate::message::protocol::Protocol;

    use bip_util::bt::{self, InfoHash, PeerId};

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
    fn positive_decode_zero_bytes_protocol() {
        let mut buffer = Vec::new();

        let exp_protocol = Protocol::Custom(Vec::new());
        let exp_extensions = any_extensions();
        let exp_hash = any_info_hash();
        let exp_pid = any_peer_id();

        let exp_message =
            HandshakeMessage::from_parts(exp_protocol.clone(), exp_extensions, exp_hash, exp_pid);

        exp_protocol.write_bytes(&mut buffer).unwrap();
        exp_extensions.write_bytes(&mut buffer).unwrap();
        buffer.write_all(exp_hash.as_ref()).unwrap();
        buffer.write_all(exp_pid.as_ref()).unwrap();

        let recv_message = HandshakeMessage::from_bytes(&buffer).unwrap().1;

        assert_eq!(exp_message, recv_message);
    }

    #[test]
    fn positive_many_bytes_protocol() {
        let mut buffer = Vec::new();

        let exp_protocol = Protocol::Custom(b"My Protocol".to_vec());
        let exp_extensions = any_extensions();
        let exp_hash = any_info_hash();
        let exp_pid = any_peer_id();

        let exp_message =
            HandshakeMessage::from_parts(exp_protocol.clone(), exp_extensions, exp_hash, exp_pid);

        exp_protocol.write_bytes(&mut buffer).unwrap();
        exp_extensions.write_bytes(&mut buffer).unwrap();
        buffer.write_all(exp_hash.as_ref()).unwrap();
        buffer.write_all(exp_pid.as_ref()).unwrap();

        let recv_message = HandshakeMessage::from_bytes(&buffer).unwrap().1;

        assert_eq!(exp_message, recv_message);
    }

    #[test]
    fn positive_bittorrent_protocol() {
        let mut buffer = Vec::new();

        let exp_protocol = Protocol::BitTorrent;
        let exp_extensions = any_extensions();
        let exp_hash = any_info_hash();
        let exp_pid = any_peer_id();

        let exp_message =
            HandshakeMessage::from_parts(exp_protocol.clone(), exp_extensions, exp_hash, exp_pid);

        exp_protocol.write_bytes(&mut buffer).unwrap();
        exp_extensions.write_bytes(&mut buffer).unwrap();
        buffer.write_all(exp_hash.as_ref()).unwrap();
        buffer.write_all(exp_pid.as_ref()).unwrap();

        let recv_message = HandshakeMessage::from_bytes(&buffer).unwrap().1;

        assert_eq!(exp_message, recv_message);
    }

    #[test]
    #[should_panic]
    fn negative_create_overflow_protocol() {
        let overflow_protocol = Protocol::Custom(vec![0u8; 256]);

        HandshakeMessage::from_parts(
            overflow_protocol,
            any_extensions(),
            any_info_hash(),
            any_peer_id(),
        );
    }
}
