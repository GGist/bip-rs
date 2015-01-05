//! Facilitates communication with a remote peer.

use std::u32;
use std::collections::{Bitv};
use std::io::net::tcp::{TcpStream};
use std::io::{IoResult, IoError, InvalidInput, BufferedStream, TimedOut, Closed};

use util::{UPeerID, SPeerID, Choice};
use self::state::{PeerState, StateSender, StateChange};
use self::state::StateChange::{Choke, Unchoke, Interested, Uninterested};
use self::data::{DataSender};

pub mod data;
pub mod handshake;
pub mod state;

const ASYNC_READ_TIMEOUT: u64 = 1;

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

/// Denotes an action that was carried out in response to a message received
/// by a remote peer.
pub enum PeerAction {
    /// A message was received that has been hidden from the user (not guaranteed to propagate).
    Hidden,
    /// A state message was received: (state_change)
    StateUpdate(StateChange),
    /// A have message was received: (piece_index)
    StateHave(u32),
    /// A request message was received: (piece_index, block_offset, block_length)
    DataRequested(u32, u32, u32),
    /// A piece message was received: (piece_index, block_offset, block_data)
    DataArrived(u32, u32, Vec<u8>),
    /// A cancel message was received: (piece_index, block_offset, block_length)
    DataCanceled(u32, u32, u32)
}

impl Peer {
    pub fn remote_peer_id(&self) -> &UPeerID {
        self.remote_id.as_slice()
    }

    pub fn receive_messages(&mut self, actions: &mut [PeerAction]) -> IoResult<uint> {
        let mut index = 0;
    
        while index < actions.len() {
            self.conn_buf.get_mut().set_read_timeout(Some(ASYNC_READ_TIMEOUT));
            let message_length = match self.conn_buf.read_be_u32() {
                Ok(n)  => n,
                Err(e) => { // TODO: Change To Pattern Guard When Compiler Supports Bind-By-Move Into Pattern Guard
                    if e.kind == TimedOut {
                        // No More Messages In Buffer
                        return Ok(index)
                    } else {
                        try!(self.close_stream());
                        return Err(e)
                    }
                }
            };
            
            // They Sent Us A Keep-Alive Message
            if message_length == 0 {
                continue;
            }
            
            self.conn_buf.get_mut().set_read_timeout(Some(ASYNC_READ_TIMEOUT));
            let message_id = try!(self.conn_buf.read_u8());
            let payload_len = message_length - 1;
            
            let peer_action = match message_id {
                state::CHOKE_ID        => try!(self.receive_state(Choke, payload_len)),
                state::UNCHOKE_ID      => try!(self.receive_state(Unchoke, payload_len)),
                state::INTERESTED_ID   => try!(self.receive_state(Interested, payload_len)),
                state::UNINTERESTED_ID => try!(self.receive_state(Uninterested, payload_len)),
                state::HAVE_ID         => try!(self.receive_have(payload_len)),
                state::BITFIELD_ID     => try!(self.receive_bitfield(payload_len)),
                data::REQUEST_ID       => try!(self.receive_request(payload_len)),
                data::PIECE_ID         => try!(self.receive_block(payload_len)),
                data::CANCEL_ID        => try!(self.receive_cancel(payload_len)),
                _                      => { self.conn_buf.consume(payload_len as uint); PeerAction::Hidden } // As Per The Spec, Ignore Other Message IDs
            };
            
            if let PeerAction::Hidden = peer_action {
                continue;
            }
            
            actions[index] = peer_action;
            index += 1;
        }
        
        Ok(index)
    }
    
    fn receive_state(&mut self, state: StateChange, payload_len: u32) -> IoResult<PeerAction> {
        if payload_len != state::STATE_PAYLOAD_LEN {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Invalid Payload Length For State Message", detail: None })
        }
    
        match state {
            Choke        => self.self_state.choked = true,
            Unchoke      => self.self_state.choked = false,
            Interested   => self.self_state.interested = true,
            Uninterested => self.self_state.interested = false
        };
        
        Ok(PeerAction::StateUpdate(state))
    }
    
    fn receive_have(&mut self, payload_len: u32) -> IoResult<PeerAction> {
        if payload_len != state::HAVE_PAYLOAD_LEN {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Invalid Payload Length For Have Message", detail: None })
        }
        
        let bitfield_len = match self.remote_pieces {
            Choice::One(ref bitfield) => bitfield.len(),
            Choice::Two(num_bits) => {
                self.remote_pieces = Choice::One(Bitv::with_capacity(num_bits as uint));
                return self.receive_have(payload_len)
            }
        };
        
        let piece_index = try!(self.conn_buf.read_be_u32());
        if piece_index >= bitfield_len as u32 {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Out Of Bounds Piece Index For Have Message", detail: None })
        }
        
        // TODO: This will always pass, refactor to resolve borrowing issues and make code clearer
        if let Choice::One(ref mut bitfield) = self.remote_pieces {
            bitfield.set(piece_index as uint, true);
        }
        
        Ok(PeerAction::StateHave(piece_index))
    }
    
    fn receive_bitfield(&mut self, payload_len: u32) -> IoResult<PeerAction> {
        // Allow clients to send 'incomplete' bitfields to save bandwidth
        
        // Make sure bitfield message was not sent twice and not after a have message
        let num_bits: u32 = match self.remote_pieces {
            Choice::One(_) => {
                try!(self.close_stream());
                return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Duplicate Or Late Bitfield Message", detail: None })
            },
            Choice::Two(num_bits) => num_bits
        };
        
        // Make sure payload is not bigger than expected
        let min_bits_sent: u32 = (payload_len - 1) * 8 + 1; // 8 Bits Per Byte
        if payload_len == 0 || min_bits_sent > num_bits {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Bitfield Message Of Invalid Length", detail: None })
        }
        
        let remote_bitfield = try!(self.conn_buf.read_exact(payload_len as uint));
        let mut bitfield = Bitv::from_bytes(remote_bitfield.as_slice());
        
        // Because unused bits could be inside last byte
        bitfield.truncate(num_bits as uint);
        
        self.remote_pieces = Choice::One(bitfield);
        
        Ok(PeerAction::Hidden)
    }
    
    fn receive_request(&mut self, payload_len: u32) -> IoResult<PeerAction> {
        if payload_len != data::REQUEST_PAYLOAD_LEN {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Invalid Payload Length For Request Message", detail: None })
        }
        
        let piece_index = try!(self.conn_buf.read_be_u32());
        let num_pieces = match self.remote_pieces {
            Choice::One(ref bitfield) => bitfield.len() as u32,
            Choice::Two(num_bits)     => num_bits
        };
        
        if piece_index >= num_pieces {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Out Of Bounds Piece Index", detail: None })
        }
        
        let block_offset = try!(self.conn_buf.read_be_u32());
        let block_len = try!(self.conn_buf.read_be_u32());
    
        Ok(PeerAction::DataRequested(piece_index, block_offset, block_len))
    }
    
    fn receive_block(&mut self, payload_len: u32) -> IoResult<PeerAction> {
        if payload_len < data::BASE_PIECE_PAYLOAD_LEN {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Invalid Payload Length For Block ('Piece') Message", detail: None })
        }
        
        let piece_index = try!(self.conn_buf.read_be_u32());
        let num_pieces = match self.remote_pieces {
            Choice::One(ref bitfield) => bitfield.len() as u32,
            Choice::Two(num_bits)     => num_bits
        };
        
        if piece_index >= num_pieces {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Out Of Bounds Piece Index", detail: None })
        }
        
        let block_offset = try!(self.conn_buf.read_be_u32());
        let block_data = try!(self.conn_buf.read_exact((payload_len - 8) as uint));
        
        Ok(PeerAction::DataArrived(piece_index, block_offset, block_data))
    }
    
    fn receive_cancel(&mut self, payload_len: u32) -> IoResult<PeerAction> {
        if payload_len != data::CANCEL_PAYLOAD_LEN {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Invalid Payload Length For Cancel Message", detail: None })
        }
        
        let piece_index = try!(self.conn_buf.read_be_u32());
        let num_pieces = match self.remote_pieces {
            Choice::One(ref bitfield) => bitfield.len() as u32,
            Choice::Two(num_bits)     => num_bits
        };
        
        if piece_index >= num_pieces {
            try!(self.close_stream());
            return Err(IoError{ kind: Closed, desc: "Remote Peer Sent Out Of Bounds Piece Index", detail: None })
        }
        
        let block_offset = try!(self.conn_buf.read_be_u32());
        let block_len = try!(self.conn_buf.read_be_u32());
        
        Ok(PeerAction::DataCanceled(piece_index, block_offset, block_len))
    }
    
    fn close_stream(&mut self) -> IoResult<()> {
        try!(self.conn_buf.get_mut().close_read());
        try!(self.conn_buf.get_mut().close_write());
        
        Ok(())
    }
}

impl StateSender for Peer {
    fn send_state(&mut self, state: StateChange) -> IoResult<()> {
        let (message_id, commit_state) = match state {
            StateChange::Choke        => (state::CHOKE_ID,   |p: &mut Peer| { p.remote_state.choked = true; }),
            StateChange::Unchoke      => (state::UNCHOKE_ID, |p: &mut Peer| { p.remote_state.choked = false;}),
            StateChange::Interested   => (state::INTERESTED_ID,   |p: &mut Peer| { p.remote_state.interested = true; }),
            StateChange::Uninterested => (state::UNINTERESTED_ID, |p: &mut Peer| { p.remote_state.interested = false; })
        };
        
        try!(self.conn_buf.write_be_u32(1));
        try!(self.conn_buf.write_u8(message_id));
        
        try!(self.conn_buf.flush());
        
        // If call to flush() fails, state will not change
        Ok(commit_state(self))
    }

    fn send_have(&mut self, piece: u32) -> IoResult<()> {
        try!(self.conn_buf.write_be_u32(5));
        try!(self.conn_buf.write_u8(state::HAVE_ID));
        try!(self.conn_buf.write_be_u32(piece));
        
        self.conn_buf.flush()
    }
    
    fn send_bitfield(&mut self, pieces: &[u8]) -> IoResult<()> {
        if pieces.len() + 1 > u32::MAX as uint {
            return Err(IoError{ kind: InvalidInput, desc: "Length Of pieces Is Too Big For Bitfield Payload", detail: None })
        }
        
        try!(self.conn_buf.write_be_u32(1 + pieces.len() as u32));
        try!(self.conn_buf.write_u8(state::BITFIELD_ID));
        try!(self.conn_buf.write(pieces));
        
        self.conn_buf.flush()
    }
}

impl DataSender for Peer {
    fn send_request(&mut self, piece: u32, offset: u32, length: u32) -> IoResult<()> {
        let num_pieces = match self.remote_pieces {
            Choice::One(ref bitfield) => bitfield.len() as u32,
            Choice::Two(num_bits)     => num_bits
        };
        
        if piece >= num_pieces {
            return Err(IoError{ kind: InvalidInput, desc: "Piece Index Out Of Bounds", detail: None })
        }
        
        try!(self.conn_buf.write_be_u32(1 + data::REQUEST_PAYLOAD_LEN));
        try!(self.conn_buf.write_u8(data::REQUEST_ID));
        try!(self.conn_buf.write_be_u32(piece));
        try!(self.conn_buf.write_be_u32(offset));
        try!(self.conn_buf.write_be_u32(length));
        
        self.conn_buf.flush()
    }

    fn send_block(&mut self, piece: u32, offset: u32, block: &[u8]) -> IoResult<()> {
        let num_pieces = match self.remote_pieces {
            Choice::One(ref bitfield) => bitfield.len() as u32,
            Choice::Two(num_bits)     => num_bits
        };
        
        if piece >= num_pieces {
            return Err(IoError{ kind: InvalidInput, desc: "Piece Index Out Of Bounds", detail: None })
        }
        
        if block.len() > u32::MAX as uint {
            return Err(IoError{ kind: InvalidInput, desc: "Block Size Is WAY TOO BIG!!!", detail: None })
        }
        
        try!(self.conn_buf.write_be_u32(1 + data::BASE_PIECE_PAYLOAD_LEN + block.len() as u32));
        try!(self.conn_buf.write_u8(data::PIECE_ID));
        try!(self.conn_buf.write_be_u32(piece));
        try!(self.conn_buf.write_be_u32(offset));
        try!(self.conn_buf.write(block));
        
        self.conn_buf.flush()
    }

    fn send_cancel(&mut self, piece: u32, offset: u32, length: u32) -> IoResult<()> {
        let num_pieces = match self.remote_pieces {
            Choice::One(ref bitfield) => bitfield.len() as u32,
            Choice::Two(num_bits)     => num_bits
        };
        
        if piece >= num_pieces {
            return Err(IoError{ kind: InvalidInput, desc: "Piece Index Out Of Bounds", detail: None })
        }
        
        try!(self.conn_buf.write_be_u32(1 + data::CANCEL_PAYLOAD_LEN));
        try!(self.conn_buf.write_u8(data::CANCEL_ID));
        try!(self.conn_buf.write_be_u32(piece));
        try!(self.conn_buf.write_be_u32(offset));
        try!(self.conn_buf.write_be_u32(length));
        
        self.conn_buf.flush()
    }
}