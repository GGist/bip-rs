//! Module for piece revelation.

use crate::ControlMessage;
use bip_handshake::InfoHash;
use bip_peer::PeerInfo;
use bip_peer::messages::BitFieldMessage;
use bip_peer::messages::HaveMessage;

pub mod error;

mod honest;

pub use self::honest::HonestRevealModule;

/// Enumeration of revelation messages that can be sent to a revelation module.
pub enum IRevealMessage {
    /// Control message.
    Control(ControlMessage),
    /// Good piece for the given `InfoHash` was found.
    FoundGoodPiece(InfoHash, u64),
    /// Received a `BitFieldMessage`.
    ReceivedBitField(PeerInfo, BitFieldMessage),
    /// Received a `HaveMessage`.
    ReceivedHave(PeerInfo, HaveMessage),
}

/// Enumeration of revelation messages that can be received from a revelation module.
pub enum ORevealMessage {
    /// Send a `BitFieldMessage`.
    SendBitField(PeerInfo, BitFieldMessage),
    /// Send a `HaveMessage`.
    SendHave(PeerInfo, HaveMessage),
}
