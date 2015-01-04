use std::io::{IoResult};

// Message IDs for various data related peer messages.
pub const REQUEST_ID: u8 = 6;
pub const PIECE_ID: u8   = 7;
pub const CANCEL_ID: u8  = 8;

// Static payload lengths for various data messages.
pub const REQUEST_PAYLOAD_LEN: u32 = 12;
pub const BASE_PIECE_PAYLOAD_LEN: u32 = 8;
pub const CANCEL_PAYLOAD_LEN: u32 = 12;

/// A trait dealing with data based messaging between a local and remote peer.
/// This should be used as an interface for implementing the data messaging
/// interface of the Bittorrent Peer Wire Protocol.
pub trait DataSender {
    /// Sends a data request message to the remote peer.
    fn send_request(&mut self, piece: u32, offset: u32, length: u32) -> IoResult<()>;
    
    /// Sends a block ("piece") request to the remote peer.
    fn send_block(&mut self, piece: u32, offset: u32, block: &[u8]) -> IoResult<()>;
    
    /// Sends a cancel request to the remote peer.
    fn send_cancel(&mut self, piece: u32, offset: u32, length: u32) -> IoResult<()>;
}