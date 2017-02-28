use std::io;
use std::u8;
use std::io::Write;

use bittorrent::handshake::extension_bits::ExtensionBits;

use bip_util::bt::{self, InfoHash, PeerId};
use nom::{IResult, be_u8};

pub struct HandshakeMessage {
    prot: String,
    ext:  ExtensionBits,
    hash: InfoHash,
    pid:  PeerId
}

impl HandshakeMessage {
    pub fn from_parts(prot: String, ext: ExtensionBits, hash: InfoHash, pid: PeerId) -> HandshakeMessage {
        if prot.len() > u8::max_value() as usize {
            panic!("bip_handshake: Handshake Message With Protocol Length Greater Than {} Found", u8::max_value())
        }

        HandshakeMessage{ prot: prot, ext: ext, hash: hash, pid: pid }
    }

    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], HandshakeMessage> {
        parse_remote_handshake(bytes)
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        try!(writer.write_all(&[self.prot.len() as u8][..]));
        try!(writer.write_all(self.prot.as_bytes()));

        try!(self.ext.write_bytes(&mut writer));

        try!(writer.write_all(self.hash.as_ref()));

        try!(writer.write_all(self.pid.as_ref()));

        Ok(())
    }
}

pub fn parse_remote_handshake(bytes: &[u8]) -> IResult<&[u8], HandshakeMessage> {
    do_parse!(bytes,
        prot: call!(parse_remote_protocol)     >>
        ext:  call!(ExtensionBits::from_bytes) >>
        hash: call!(parse_remote_hash)         >>
        pid:  call!(parse_remote_pid)          >>
        (HandshakeMessage::from_parts(prot.to_string(), ext, hash, pid))
    )
}

fn parse_remote_protocol(bytes: &[u8]) -> IResult<&[u8], &str> {
    do_parse!(bytes,
        length:   be_u8                   >>
        protocol: take_str!(length) >>
        (protocol)
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