use std::io;

use bip_util::bt::{self, PeerId, InfoHash};
use nom::{IResult, be_u8};

pub fn parse_remote_handshake(bytes: &[u8],
                              expected_pid: Option<PeerId>,
                              expected_protocol: &'static str)
                              -> io::Result<(InfoHash, PeerId)> {
    let parse_result = chain!(bytes,
        _unused_prot: call!(parse_remote_protocol, expected_protocol) ~
        _unused_ext:  take!(8) ~
        hash:         call!(parse_remote_hash) ~
        pid:          call!(parse_remote_pid, expected_pid) ,
        || { (hash, pid) }
    );

    match parse_result {
        IResult::Done(_, (hash, pid)) => Ok((hash, pid)),
        IResult::Error(_) |
        IResult::Incomplete(_) => Err(io::Error::new(io::ErrorKind::ConnectionAborted, "Protocol Parsing Error")),
    }
}

pub fn parse_remote_protocol<'a>(bytes: &'a [u8], expected_protocol: &'static str) -> IResult<&'a [u8], &'a [u8]> {
    let expected_length = expected_protocol.len() as u8;

    switch!(bytes, map!(be_u8, |len| len == expected_length),
        true => tag!(expected_protocol.as_bytes())
    )
}

pub fn parse_remote_hash(bytes: &[u8]) -> IResult<&[u8], InfoHash> {
    map!(bytes, take!(bt::INFO_HASH_LEN), |hash| InfoHash::from_hash(hash).unwrap())
}

pub fn parse_remote_pid(bytes: &[u8], opt_expected_pid: Option<PeerId>) -> IResult<&[u8], PeerId> {
    if let Some(expected_pid) = opt_expected_pid {
        map!(bytes, tag!(expected_pid.as_ref()), |id| PeerId::from_hash(id).unwrap())
    } else {
        map!(bytes, take!(bt::PEER_ID_LEN), |id| PeerId::from_hash(id).unwrap())
    }
}
