#![allow(unused)]

use std::net::SocketAddr;
use std::sync::mpsc::SyncSender;

use bip_handshake::BTPeer;
use bip_util::bt::{PeerId, InfoHash};
use bip_util::send::{TrySender, SplitSender};
use rotor::Notifier;

use disk::ODiskMessage;
use piece::{OSelectorMessage, OSelectorMessageKind};
use message::standard::{HaveMessage, BitFieldMessage, RequestMessage, PieceMessage, CancelMessage};
use token::Token;

mod error;
mod tcp;
mod machine;

// ----------------------------------------------------------------------------//

/// Since peers could be connected to us over multiple connections
/// but may advertise the same peer id, we need to dis ambiguate
/// them by a combination of the address (ip + port) and peer id.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct PeerIdentifier {
    addr: SocketAddr,
    pid: PeerId,
}

impl PeerIdentifier {
    pub fn new(addr: SocketAddr, pid: PeerId) -> PeerIdentifier {
        PeerIdentifier {
            addr: addr,
            pid: pid,
        }
    }
}

// ----------------------------------------------------------------------------//

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum IProtocolMessage {
    /// Message from the disk manager to the protocol layer.
    DiskManager(ODiskMessage),
    /// Message from the piece manager to the protocol layer.
    PieceManager(OSelectorMessage),
}

impl From<IProtocolMessage> for ODiskMessage {
    fn from(data: IProtocolMessage) -> ODiskMessage {
        match data {
            IProtocolMessage::DiskManager(disk) => disk,
            IProtocolMessage::PieceManager(_) => unreachable!()
        }
    }
}

impl From<ODiskMessage> for IProtocolMessage {
    fn from(data: ODiskMessage) -> IProtocolMessage {
        IProtocolMessage::DiskManager(data)
    }
}

impl From<IProtocolMessage> for OSelectorMessage {
    fn from(data: IProtocolMessage) -> OSelectorMessage {
        match data {
            IProtocolMessage::PieceManager(piece) => piece,
            IProtocolMessage::DiskManager(_) => unreachable!()
        }
    }
}

impl From<OSelectorMessage> for IProtocolMessage {
    fn from(data: OSelectorMessage) -> IProtocolMessage {
        IProtocolMessage::PieceManager(data)
    }
}

// ----------------------------------------------------------------------------//

pub struct ProtocolSender {
    send: SyncSender<IProtocolMessage>,
    noti: Notifier,
}

impl ProtocolSender {
    pub fn new(send: SyncSender<IProtocolMessage>, noti: Notifier) -> ProtocolSender {
        ProtocolSender {
            send: send,
            noti: noti,
        }
    }
}

impl<T> TrySender<T> for ProtocolSender
    where T: Into<IProtocolMessage> + Send + From<IProtocolMessage>
{
    fn try_send(&self, data: T) -> Option<T> {
        let ret = TrySender::try_send(&self.send, data.into()).map(|data| data.into());
        
        if ret.is_none() {
            self.noti
                .wakeup()
                .expect("bip_peer: ProtocolSender Failed To Send Wakeup");
        }

        ret
    }
}

impl Clone for ProtocolSender {
    fn clone(&self) -> ProtocolSender {
        ProtocolSender {
            send: self.send.clone(),
            noti: self.noti.clone(),
        }
    }
}

// ----------------------------------------------------------------------------//

pub struct OProtocolMessage {
    kind: OProtocolMessageKind,
    id: PeerIdentifier,
}

impl OProtocolMessage {
    pub fn new(id: PeerIdentifier, kind: OProtocolMessageKind) -> OProtocolMessage {
        OProtocolMessage {
            kind: kind,
            id: id,
        }
    }

    pub fn destroy(self) -> (PeerIdentifier, OProtocolMessageKind) {
        (self.id, self.kind)
    }
}

pub enum OProtocolMessageKind {
    /// Message that a peer has connected for the given InfoHash.
    PeerConnect(Box<TrySender<OSelectorMessage>>, InfoHash),
    /// Message that a peer has disconnected.
    PeerDisconnect,
    /// Message that a peer has choked us.
    PeerChoke,
    /// Message that a peer has unchoked us.
    PeerUnChoke,
    /// Message that a peer is interested in us.
    PeerInterested,
    /// Message that a peer is not interested in us.
    PeerUnInterested,
    /// Message that a peer has a specific piece.
    PeerHave(HaveMessage),
    /// Message that a peer has all pieces in the bitfield.
    PeerBitField(BitFieldMessage),
    /// Message that a peer has request a block from us.
    PeerRequest(RequestMessage),
    /// Message that a peer has sent a block to us.
    PeerPiece(Token, PieceMessage),
    /// Message that a peer has cancelled a block request from us.
    PeerCancel(CancelMessage),
}