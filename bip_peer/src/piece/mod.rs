#![allow(unused)]

use bip_util::sender::{Sender};
use mio::{self};

use disk::{ODiskResponse};
use protocol::{PeerIdentifier, OProtocolMessage};
use message::standard::{HaveMessage, BitfieldMessage, RequestMessage, PieceMessage};
use token::{Token};

enum IPieceMessage {
    /// Message from the disk manager to the piece selector.
    DiskManager(ODiskResponse),
    /// Message from the protocol layer to the piece selector.
    ///
    /// Token is used to pin this message to a given channel.
    Protocol(Token, OProtocolMessage)
}

struct DiskSender {
    send: mio::Sender<IPieceMessage>
}

impl Sender<ODiskResponse> for DiskSender {
    fn send(&self, data: ODiskResponse) {
        self.send.send(IPieceMessage::DiskManager(data));
    }
}

struct ProtocolSender {
    id:   Token,
    send: mio::Sender<IPieceMessage>
}

impl Sender<OProtocolMessage> for ProtocolSender {
    fn send(&self, data: OProtocolMessage) {
        self.send.send(IPieceMessage::Protocol(self.id, data));
    }
}

//----------------------------------------------------------------------------//

/// Outgoing piece message to the protocol layer.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct OPieceMessage {
    kind: OPieceMessageKind,
    id:   PeerIdentifier
}

impl OPieceMessage {
    pub fn new(id: PeerIdentifier, kind: OPieceMessageKind) -> OPieceMessage {
        OPieceMessage{ kind: kind, id: id }
    }
    
    pub fn id(&self) -> PeerIdentifier {
        self.id
    }
    
    pub fn kind(&self) -> OPieceMessageKind {
        self.kind
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum OPieceMessageKind {
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
    PeerBitfield(BitfieldMessage),
    /// Message to send a peer block request.
    PeerRequest(RequestMessage),
    /// Message to send a block to a peer.
    PeerPiece(Token, PieceMessage)
}