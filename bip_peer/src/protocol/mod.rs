#![allow(unused)]

use std::net::{SocketAddr};

use bip_handshake::{BTPeer};
use bip_util::bt::{PeerId, InfoHash};
use bip_util::sender::{Sender};

use disk::{ODiskResponse};
use message::standard::{HaveMessage, BitfieldMessage, RequestMessage, PieceMessage};
use piece::{OPieceMessage};
use token::{Token};

//----------------------------------------------------------------------------//

/// Since peers could be connected to us over multiple connections
/// but may advertise the same peer id, we need to dis ambiguate
/// them by a combination of the address (ip + port) and peer id.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct PeerIdentifier {
    addr: SocketAddr,
    pid:  PeerId
}

//----------------------------------------------------------------------------//

/// Incoming protocol message from some other component.
///
/// The value of P depends on the protocol in use as different
/// protocols can talk to different types of peers.
enum IProtocolMessage<P> {
    /// Message from the handshaker to the protocol layer.
    Handshaker(P),
    /// Message from the disk manager to the protocol layer.
    DiskManager(ODiskResponse),
    /// Message from the piece manager to the protocol layer.
    PieceManager(OPieceMessage)
}

struct PeerSender {
    send: Sender<IProtocolMessage<BTPeer>>
}

impl Sender<BTPeer> for PeerSender {
    fn send(&self, data: BTPeer) {
        self.send.send(IProtocolMessage::Handshaker(data));
    }
}

struct DiskSender {
    send: Sender<IProtocolMessage<BTPeer>>
}

impl Sender<ODiskResponse> for DiskSender {
    fn send(&self, data: ODiskResponse) {
        self.send.send(IProtocolMessage::DiskManager(data));
    }
}

struct PieceSender {
    send: Sender<IProtocolMessage<BTPeer>>
}

impl Sender<OPieceMessage> for PieceSender {
    fn send(&self, data: OPieceMessage) {
        self.send.send(IProtocolMessage::PieceManager(data))
    }
}

//----------------------------------------------------------------------------//

/// Outgoing protocol message to some other component.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct OProtocolMessage {
    kind: OProtocolMessageKind,
    id:   PeerIdentifier
}

impl OProtocolMessage {
    pub fn new(id: PeerIdentifier, kind: OProtocolMessageKind) -> OProtocolMessage {
        OProtocolMessage{ kind: kind, id: id }
    }
    
    pub fn id(&self) -> PeerIdentifier {
        self.id
    }
    
    pub fn kind(&self) -> OProtocolMessageKind {
        self.kind
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum OProtocolMessageKind {
    /// Message that a peer has connected for the given InfoHash.
    PeerConnect(InfoHash),
    /// Message that a peer has disconnected.
    PeerDisconnect,
    /// Message that a peer has choked us.
    PeerChoke,
    /// Message that a peer has unchoked us.
    PeerUnChoke,
    /// Message that a peer is interested in us.
    PeerInterested,
    /// Message that a peer is not interested in us.
    PeerNotInterested,
    /// Message that a peer has a specific piece.
    PeerHave(HaveMessage),
    /// Message that a peer has all pieces in the bitfield.
    PeerBitfield(BitfieldMessage),
    /// Message that a peer has request a block from us.
    PeerRequest(RequestMessage),
    /// Message that a peer has sent a block to us.
    PeerPiece(Token, PieceMessage)
}