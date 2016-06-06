#![allow(unused)]

use std::sync::mpsc::SyncSender;

use bip_util::sender::{Sender, PrioritySender};
use rotor::Notifier;

use disk::ODiskMessage;
use protocol::{PeerIdentifier, OProtocolMessage};
use message::standard::{HaveMessage, BitFieldMessage, RequestMessage, PieceMessage, CancelMessage};
use token::Token;

mod selectors;

pub use piece::selectors::PieceSelector;

pub enum ISelectorMessage {
    /// Message from the disk manager to the piece selector.
    DiskManager(ODiskMessage),
    /// Message from the protocol layer to the piece selector.
    ///
    /// Token is used to pin this message to a given channel.
    Protocol(Token, OProtocolMessage),
}

impl From<ODiskMessage> for ISelectorMessage {
    fn from(data: ODiskMessage) -> ISelectorMessage {
        ISelectorMessage::DiskManager(data)
    }
}

// ----------------------------------------------------------------------------//

pub struct SelectorSender {
    id: Token,
    send: SyncSender<ISelectorMessage>,
    noti: Notifier,
}

impl<T> Sender<T> for SelectorSender
    where T: Into<ISelectorMessage> + Send
{
    fn send(&self, data: T) {
        self.send
            .send(data.into())
            .expect("bip_peer: SelectorSender failed to send message");

        self.noti
            .wakeup()
            .expect("bip_peer: SelectorSender failed to send wakup");
    }
}

// Have to specialize the impl for protocol messages so we can insert the token
impl Sender<OProtocolMessage> for SelectorSender {
    fn send(&self, data: OProtocolMessage) {
        self.send
            .send(ISelectorMessage::Protocol(self.id, data))
            .expect("bip_peer: SelectorSender failed to send message");

        self.noti
            .wakeup()
            .expect("bip_peer: SelectorSender failed to send wakup");
    }
}

// ----------------------------------------------------------------------------//

/// Outgoing piece message to the protocol layer.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct OSelectorMessage {
    kind: OSelectorMessageKind,
    id: PeerIdentifier,
}

impl OSelectorMessage {
    pub fn new(id: PeerIdentifier, kind: OSelectorMessageKind) -> OSelectorMessage {
        OSelectorMessage {
            kind: kind,
            id: id,
        }
    }

    pub fn id(&self) -> PeerIdentifier {
        self.id
    }

    pub fn kind(&self) -> OSelectorMessageKind {
        self.kind.clone()
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum OSelectorMessageKind {
    /// Message to keep alive the connection.
    ///
    /// Selector can, but does not have to, send this message.
    PeerKeepAlive,
    /// Message to disconnect from the peer.
    PeerDisconnect,
    /// Message to send a peer choke.
    PeerChoke,
    /// Message to send a peer unchoke.
    PeerUnChoke,
    /// Message to send a peer interested.
    PeerInterested,
    /// Message to send a peer not interested.
    PeerNotInterested,
    /// Message to send a peer have.
    PeerHave(HaveMessage),
    /// Message to send a peer bitfield.
    PeerBitField(BitFieldMessage),
    /// Message to send a peer block request.
    PeerRequest(RequestMessage),
    /// Message to send a block to a peer.
    PeerPiece(Token, PieceMessage),
    /// Message to send a block cancel to a peer.
    PeerCancel(CancelMessage),
}
