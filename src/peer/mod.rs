//! Facilitates communication with a remote peer.

use std::default::{Default};
use std::collections::{Bitv};
use std::io::net::tcp::{TcpStream};
use std::io::{IoResult, IoError, BufferedStream, TimedOut, Closed};
use peer::message::{BlockLength, PeerMessage, PeerReader, PeerWriter, StateChange, PieceIndex, BlockOffset};
use peer::block::{Block};
use util::{SPeerID, Choice};

pub mod block;
pub mod handshake;
pub mod message;

const ASYNC_READ_TIMEOUT: u64 = 1;

/// Represents the state associated with one side of a peer connection.
#[allow(dead_code)]
pub struct PeerState {
    pub choked:     bool,
    pub interested: bool
}

impl Copy for PeerState { }
impl Default for PeerState {
    fn default() -> PeerState {
        PeerState{ choked: false, interested: false }
    }
}

/// A Peer object representing a connection to a remote peer as well as the
/// bittorrent state associated with that connection. Since the messaging
/// interface between a peer is asynchronous, peers will start out as not
/// choked and not interested.
pub struct Peer {
    conn_buf:      BufferedStream<TcpStream>,
    self_state:    PeerState,
    remote_state:  PeerState,
    remote_id:     SPeerID,
    remote_pieces: Choice<Bitv, u32> // Bitfield Or Number Of Pieces
}

impl Peer {
    /// Returns the state associated with our end of the connection.
    pub fn local_state(&self) -> PeerState {
        self.self_state
    }
    
    /// Returns the state associated with the remote end of the connection.
    pub fn remote_state(&self) -> PeerState {
        self.remote_state
    }

    /// Returns a reference to a buffer containing the peer id of the remote peer.
    pub fn remote_peer_id(&self) -> &SPeerID {
        &self.remote_id
    }
    
    /// Processes all of the messages sent to us from the remote peer.
    ///
    /// This method will not block while waiting for messages to be sent to us, but
    /// it will block if only part of a message was sent to us so far.
    pub fn process_messages<'a, T>(&mut self, messages: &mut [PeerMessage], block: &mut T) -> IoResult<()>
        where T: FnMut<(BlockLength,), &'a mut Block> {
        let mut emptied_messages = false;
        let mut message_index = 0;
        
        let num_pieces = self.num_pieces();
        while !emptied_messages {
            self.conn_buf.get_mut().set_read_timeout(Some(ASYNC_READ_TIMEOUT));
            
            match self.conn_buf.read_message(num_pieces, block) {
                Ok(peer_message) => { messages[message_index] = peer_message; },
                Err(e)           => {
                    if e.kind == TimedOut {
                        emptied_messages = true;
                    } else {
                        return Err(IoError{ kind: Closed, desc: "Connection To Peer Closed", detail: None })
                    }
                }
            }
            
            message_index += 1;
        }
        
        Ok(())
    }
    
    /// Sends a message to the remote peer telling them that we are changing their state.
    pub fn change_state(&mut self, state: StateChange) -> IoResult<()> {
        match self.conn_buf.write_state(state) {
            Err(e) => { try!(self.close_stream()); return Err(e) },
            Ok(_)  => ()
        };
        
        self.conn_buf.flush()
    }
    
    /// Sends a message to the remote peer telling them that we have successfully
    /// downloaded and verified the hash of piece.
    pub fn notify_have(&mut self, piece: PieceIndex) -> IoResult<()> {
        match self.conn_buf.write_have(piece) {
            Err(e) => { try!(self.close_stream()); return Err(e) },
            Ok(_)  => ()
        };
        
        self.conn_buf.flush()
    }
    
    /// Sends a message to the remote peer telling them that the bits set in each byte
    /// correspond to the pieces we have downloaded.
    ///
    /// To save bandwidth, partial bitfields combined with have messages to fill in
    /// spread out piece indices are generally allowed.
    pub fn notify_bitfield(&mut self, bitfield: &[u8]) -> IoResult<()> {
        match self.conn_buf.write_bitfield(bitfield) {
            Err(e) => { try!(self.close_stream()); return Err(e) },
            Ok(_)  => ()
        };
        
        self.conn_buf.flush()
    }
    
    /// Sends a message to the remote peer telling them that we are requesting a specific
    /// block of a specific length of a designated piece.
    ///
    /// This is a data-oriented message and should not be sent if the peer is choking 
    /// us (local end of connection has choked set to true).
    pub fn request_block(&mut self, piece: PieceIndex, offset: BlockOffset, len: BlockLength) -> IoResult<()> {
        match self.conn_buf.write_request(piece, offset, len) {
            Err(e) => { try!(self.close_stream()); return Err(e) },
            Ok(_)  => ()
        };
        
        self.conn_buf.flush()
    }
    
    /// Sends a message to the remote peer telling them that we are cancelling a prior
    /// request for a block.
    ///
    /// This is a data-oriented message and should not be sent if the peer is choking 
    /// us (local end of connection has choked set to true).
    pub fn cancel_block(&mut self, piece: PieceIndex, offset: BlockOffset, len: BlockLength) -> IoResult<()> {
        match self.conn_buf.write_cancel(piece, offset, len) {
            Err(e) => { try!(self.close_stream()); return Err(e) },
            Ok(_)  => ()
        };
        
        self.conn_buf.flush()
    }
    
    /// Sends a message to the remote peer telling them that we are sending a block 
    /// of data to them.
    ///
    /// This is a data-oriented message and should not be sent if the peer is choking 
    /// us (local end of connection has choked set to true).
    pub fn send_block(&mut self, piece: PieceIndex, offset: BlockOffset, block_data: &[u8]) -> IoResult<()> {
        match self.conn_buf.write_block(piece, offset, block_data) {
            Err(e) => { try!(self.close_stream()); return Err(e) },
            Ok(_)  => ()
        };
        
        self.conn_buf.flush()
    }
    
    fn num_pieces(&self) -> u32 {
        match self.remote_pieces {
            Choice::One(ref bitfield) => bitfield.len() as u32,
            Choice::Two(num_bits)    => num_bits
        }
    }
    
    fn close_stream(&mut self) -> IoResult<()> {
        try!(self.conn_buf.get_mut().close_read());
        try!(self.conn_buf.get_mut().close_write());
        
        Ok(())
    }
}