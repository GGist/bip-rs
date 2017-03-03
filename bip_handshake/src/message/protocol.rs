use std::io;
use std::u8;
use std::io::Write;

use bip_util::bt::{self, InfoHash, PeerId};
use nom::{IResult, be_u8, rest};

const BITTORRENT_10_PROTOCOL:     &'static [u8] = b"BitTorrent Protocol";
const BITTORRENT_10_PROTOCOL_LEN: u8            = 19;

/// Protocol information transmitted as part of the handshake.
#[derive(Clone, PartialEq, Eq)]
pub enum Protocol {
    BitTorrent,
    Custom(Vec<u8>)
}

impl Protocol {
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], Protocol> {
        parse_protocol(bytes)
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        let (len, bytes) = match self {
            &Protocol::BitTorrent       => (BITTORRENT_10_PROTOCOL_LEN as usize, &BITTORRENT_10_PROTOCOL[..]),
            &Protocol::Custom(ref prot) => (prot.len(), &prot[..])
        };

        try!(writer.write_all(&[len as u8][..]));
        try!(writer.write_all(bytes));

        Ok(())
    }
}

fn parse_protocol(bytes: &[u8]) -> IResult<&[u8], Protocol> {
    parse_real_protocol(bytes)
}

fn parse_real_protocol(bytes: &[u8]) -> IResult<&[u8], Protocol> {
    switch!(bytes, call!(parse_raw_protocol),
        BITTORRENT_10_PROTOCOL => value!(Protocol::BitTorrent) |
        custom                 => value!(Protocol::Custom(custom.to_vec()))
    )
}

fn parse_raw_protocol(bytes: &[u8]) -> IResult<&[u8], &[u8]> {
    do_parse!(bytes,
        length:       be_u8         >>
        raw_protocol: take!(length) >>
        (raw_protocol)
    )
}
