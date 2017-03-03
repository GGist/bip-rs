use std::io;
use std::u8;
use std::io::Write;

use message::protocol::Protocol;
use message::extensions::Extensions;

use bip_util::bt::{self, InfoHash, PeerId};
use nom::{IResult};

#[derive(Clone)]
pub struct HandshakeMessage {
    prot: Protocol,
    ext:  Extensions,
    hash: InfoHash,
    pid:  PeerId
}

impl HandshakeMessage {
    /// Create a new `HandshakeMessage` from the given components.
    pub fn from_parts(prot: Protocol, ext: Extensions, hash: InfoHash, pid: PeerId) -> HandshakeMessage {
        if let Protocol::Custom(ref custom) = prot {
            if custom.len() > u8::max_value() as usize {
                panic!("bip_handshake: Handshake Message With Protocol Length Greater Than {} Found", u8::max_value())
            }
        }

        HandshakeMessage{ prot: prot, ext: ext, hash: hash, pid: pid }
    }

    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], HandshakeMessage> {
        parse_remote_handshake(bytes)
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        try!(self.prot.write_bytes(&mut writer));

        try!(self.ext.write_bytes(&mut writer));

        try!(writer.write_all(self.hash.as_ref()));

        try!(writer.write_all(self.pid.as_ref()));

        Ok(())
    }

    pub fn into_parts(self) -> (Protocol, Extensions, InfoHash, PeerId) {
        (self.prot, self.ext, self.hash, self.pid)
    }
}

pub fn parse_remote_handshake(bytes: &[u8]) -> IResult<&[u8], HandshakeMessage> {
    do_parse!(bytes,
        prot: call!(Protocol::from_bytes)   >>
        ext:  call!(Extensions::from_bytes) >>
        hash: call!(parse_remote_hash)      >>
        pid:  call!(parse_remote_pid)       >>
        (HandshakeMessage::from_parts(prot, ext, hash, pid))
    )
}

fn parse_remote_hash(bytes: &[u8]) -> IResult<&[u8], InfoHash> {
    do_parse!(bytes,
        hash: take!(bt::INFO_HASH_LEN) >>
        (InfoHash::from_hash(hash).unwrap())
    )
}

fn parse_remote_pid(bytes: &[u8]) -> IResult<&[u8], PeerId> {
    do_parse!(bytes,
        pid: take!(bt::PEER_ID_LEN) >>
        (PeerId::from_hash(pid).unwrap())
    )
}