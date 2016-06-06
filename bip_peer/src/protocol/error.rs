use std::error::Error;
use std::fmt::{self, Display, Formatter};

use protocol::{PeerIdentifier};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct ProtocolError {
    id: PeerIdentifier,
    kind: ProtocolErrorKind
}

impl ProtocolError {
    pub fn new(id: PeerIdentifier, kind: ProtocolErrorKind) -> ProtocolError {
        ProtocolError{ id: id, kind: kind }
    }
}

impl Display for ProtocolError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        f.write_fmt(format_args!("Protocol Error For {:?} Caused By {:?}", self.id, self.kind))
    }
}

impl Error for ProtocolError {
    fn description(&self) -> &str {
        "Protocol Error Which Caused A Peer Disconnection"
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum ProtocolErrorKind {
    InvalidMessage,
    RemoteTimeout,
    RemoteDisconnect,
    RemoteError
}