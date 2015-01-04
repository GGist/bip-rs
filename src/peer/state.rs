use std::default::{Default};
use std::io::{IoResult};

/// Represents the state for one end of a connection between two peers.
#[derive(Copy)]
pub struct PeerState {
    pub choked: bool,
    pub interested: bool
}

impl Default for PeerState {
    fn default() -> Self {
        PeerState{ choked: false, interested: false }
    }
}

// Message IDs for various state related peer messages.
pub const CHOKE_ID: u8        = 0;
pub const UNCHOKE_ID: u8      = 1;
pub const INTERESTED_ID: u8   = 2;
pub const UNINTERESTED_ID: u8 = 3;
pub const HAVE_ID: u8         = 4;
pub const BITFIELD_ID: u8     = 5;

// Static payload lengths for various state messages.
pub const STATE_PAYLOAD_LEN: u32 = 0;
pub const HAVE_PAYLOAD_LEN: u32 = 4;

/// Represents a state change for an end of a connection in a peer to peer
/// connection.
#[derive(Copy)]
pub enum StateChange {
    Choke,
    Unchoke,
    Interested,
    Uninterested
}

/// A trait dealing with state based messaging between a local and remote peer.
/// This should be used as an interface for implementing the state messaging
/// interface of the Bittorrent Peer Wire Protocol.
pub trait StateSender {
    /// Sends an update message to the peer where the message is determined by
    /// the state parameter passed in.
    fn send_state(&mut self, state: StateChange) -> IoResult<()>;

    /// Sends a have message to the peer where the piece is the index of the
    /// piece that we have successfully downloaded AND validated.
    fn send_have(&mut self, piece: u32) -> IoResult<()>;
    
    /// Sends a bitfield message to the peer where a 1 represents a piece that
    /// we have and a 0 represents a piece we are missing. 
    ///
    /// This MUST be sent immediately after the handshake and NOT any other 
    /// time. We may choose to not send this if we have no pieces downloaded.
    fn send_bitfield(&mut self, pieces: &[u8]) -> IoResult<()>;
}