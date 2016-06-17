#![allow(unused)]

use std::net::SocketAddr;
use std::sync::mpsc::SyncSender;

use bip_handshake::BTPeer;
use bip_util::bt::{PeerId, InfoHash};
use bip_util::sender::{Sender, PrioritySender};
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

impl From<ODiskMessage> for IProtocolMessage {
    fn from(data: ODiskMessage) -> IProtocolMessage {
        IProtocolMessage::DiskManager(data)
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

impl<T: Send> Sender<T> for ProtocolSender
    where T: Into<IProtocolMessage>
{
    fn send(&self, data: T) {
        self.send
            .send(data.into())
            .expect("bip_peer: ProtocolSender failed to send message");

        self.noti
            .wakeup()
            .expect("bip_peer: ProtocolSender failed to send wakup");
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
    PeerConnect(Box<Sender<OSelectorMessage>>, InfoHash),
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
