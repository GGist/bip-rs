use std::io::{IoResult, IoError, InvalidInput};
use peer::block::{Block};

const CHOKE_ID: u8        = 0;
const UNCHOKE_ID: u8      = 1;
const INTERESTED_ID: u8   = 2;
const UNINTERESTED_ID: u8 = 3;
const HAVE_ID: u8         = 4;
const BITFIELD_ID: u8     = 5;
const REQUEST_ID: u8      = 6;
const BLOCK_ID: u8        = 7;
const CANCEL_ID: u8       = 8;

const KEEP_ALIVE_MESSAGE_LEN: u32 = 0;

const MESSAGE_ID_LEN: u32 = 1;

const STATE_MESSAGE_LEN: u32   = MESSAGE_ID_LEN;
const HAVE_MESSAGE_LEN: u32    = MESSAGE_ID_LEN + 4;
const REQUEST_MESSAGE_LEN: u32 = MESSAGE_ID_LEN + 12;
const CANCEL_MESSAGE_LEN: u32  = MESSAGE_ID_LEN + 12;

const BASE_BLOCK_MESSAGE_LEN: u32 = MESSAGE_ID_LEN + 8;

pub type PieceIndex  = u32;
pub type BlockOffset = u32;
pub type BlockLength = u32;

/// Represents a state change for one end of a connection.
#[derive(Copy, Show)]
pub enum StateChange {
    Choke,
    Unchoke,
    Interested,
    Uninterested
}

/// Represents a message received from a remote peer.
#[derive(Show)]
pub enum PeerMessage {
    /// Message type has been hidden from client.
    Hidden,
    /// Peer has changed the state for our end of the connection.
    StateUpdate(StateChange),
    /// Peer has successfully downloaded a piece.
    HaveUpdate(PieceIndex),
    /// Peer has sent us a list of pieces they currently have.
    BitfieldUpdate(Vec<u8>),
    /// Peer has sent us a request for a block.
    BlockRequest(PieceIndex, BlockOffset, BlockLength),
    /// Peer has sent us a cancel message for a block they requested previously.
    CancelRequest(PieceIndex, BlockOffset, BlockLength),
    /// Peer has sent us a block of data that we requested.
    BlockReceived(PieceIndex, BlockOffset, BlockLength),
    /// Peer has sent us a block of data that we requested that is too big to fit
    /// in the buffer provided by the client.
    BlockReceivedTooBig(PieceIndex, BlockOffset, Vec<u8>)
}

#[inline(always)]
fn get_num_bits(bytes: u32) -> u32 {
    bytes * 8
}

#[inline(always)]
fn get_payload_len(message_len: u32) -> u32 {
    message_len - MESSAGE_ID_LEN
}

#[inline(always)]
fn get_block_len(payload_len: u32) -> u32 {
    payload_len - get_payload_len(BASE_BLOCK_MESSAGE_LEN)
}

pub trait PeerReader {
    fn read_message<'a, F>(&mut self, max_pieces: u32, block: &mut F) -> IoResult<PeerMessage>
        where F: FnMut<(BlockLength,), &'a mut Block>;
    
    fn read_have(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage>;
    
    fn read_bitfield(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage>;
    
    fn read_request(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage>;
    
    fn read_cancel(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage>;
    
    fn read_block(&mut self, payload_len: u32, max_pieces: u32, block_buffer: &mut [u8]) -> IoResult<PeerMessage>;
}

impl<T: Reader> PeerReader for T {
    fn read_message<'a, F>(&mut self, max_pieces: u32, block: &mut F) ->  IoResult<PeerMessage>
        where F: FnMut<(BlockLength,), &'a mut Block> {
        let message_len = try!(self.read_be_u32());
            
        if message_len == KEEP_ALIVE_MESSAGE_LEN {
            return Ok(PeerMessage::Hidden)
        }
        
        let message_id = try!(self.read_u8());
        let payload_len = message_len - MESSAGE_ID_LEN;
        
        let message_action = match message_id {
            CHOKE_ID        => PeerMessage::StateUpdate(StateChange::Choke),
            UNCHOKE_ID      => PeerMessage::StateUpdate(StateChange::Unchoke),
            INTERESTED_ID   => PeerMessage::StateUpdate(StateChange::Interested),
            UNINTERESTED_ID => PeerMessage::StateUpdate(StateChange::Uninterested),
            HAVE_ID         => try!(self.read_have(payload_len, max_pieces)),
            BITFIELD_ID     => try!(self.read_bitfield(payload_len, max_pieces)),
            REQUEST_ID      => try!(self.read_request(payload_len, max_pieces)),
            CANCEL_ID       => try!(self.read_cancel(payload_len, max_pieces)),
            BLOCK_ID        => {
                let block = block.call_mut((get_block_len(payload_len),));
                let message_action = try!(self.read_block(payload_len, max_pieces, block.as_mut_slice()));
                
                if let PeerMessage::BlockReceived(index, offset, length) = message_action {
                    block.set_active(index, offset, length);
                }
                message_action
            },
            _               => { // Allow Unrecognize Message IDs
                let block = block.call_mut((payload_len,));
                
                if payload_len <= block.len() {
                    try!(self.read_at_least(payload_len as uint, block.as_mut_slice()));
                } else {
                    try!(self.read_exact(payload_len as uint));
                }
                return Ok(PeerMessage::Hidden)
            }
        };
        
        Ok(message_action)
    }
    
    fn read_have(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage> {
        if payload_len != get_payload_len(HAVE_MESSAGE_LEN) {
            return Err(IoError{ kind: InvalidInput, desc: "Remote Peer Sent Invalid Length For Have Message", detail: None})
        }
        
        let piece_index = try!(self.read_be_u32());
        if piece_index >= max_pieces {
            return Err(IoError{ kind: InvalidInput, desc: "Remote Peer Sent Invalid Piece Index For Have Message", detail: None})
        }
        
        Ok(PeerMessage::HaveUpdate(piece_index))
    }
    
    fn read_bitfield(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage> {
        let min_num_bits = get_num_bits(payload_len) - 7;
        if min_num_bits > max_pieces {
            return Err(IoError{ kind: InvalidInput, desc: "Remote Peer Sent Invalid Length For Bitfield Message", detail: None})
        }
        
        let bytes = try!(self.read_exact(payload_len as uint));
        
        Ok(PeerMessage::BitfieldUpdate(bytes))
    }
    
    fn read_request(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage> {
        if payload_len != get_payload_len(REQUEST_MESSAGE_LEN) {
            return Err(IoError{ kind: InvalidInput, desc: "Remote Peer Sent Invalid Length For Bitfield Message", detail: None})
        }
        
        let piece_index = try!(self.read_be_u32());
        if piece_index >= max_pieces {
            return Err(IoError{ kind: InvalidInput, desc: "Remote Peer Sent Invalid Piece Index For Request Message", detail: None})
        }
        
        let block_offset = try!(self.read_be_u32());
        let block_len = try!(self.read_be_u32());
        
        Ok(PeerMessage::BlockRequest(piece_index, block_offset, block_len))
    }
    
    fn read_cancel(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage> {
        if payload_len != get_payload_len(CANCEL_MESSAGE_LEN) {
            return Err(IoError{ kind: InvalidInput, desc: "Remote Peer Sent Invalid Length For Cancel Message", detail: None})
        }
        
        let piece_index = try!(self.read_be_u32());
        if piece_index >= max_pieces {
            return Err(IoError{ kind: InvalidInput, desc: "Remote Peer Sent Invalid Piece Index For Cancel Message", detail: None})
        }
        
        let block_offset = try!(self.read_be_u32());
        let block_len = try!(self.read_be_u32());
        
        Ok(PeerMessage::CancelRequest(piece_index, block_offset, block_len))
    }
    
    fn read_block(&mut self, payload_len: u32, max_pieces: u32, block_buffer: &mut [u8]) -> IoResult<PeerMessage> {
        if payload_len < get_payload_len(BASE_BLOCK_MESSAGE_LEN) {
            return Err(IoError{ kind: InvalidInput, desc: "Remote Peer Sent Invalid Length For Block Message", detail: None})
        }
        
        let piece_index = try!(self.read_be_u32());
        if piece_index >= max_pieces {
            return Err(IoError{ kind: InvalidInput, desc: "Remote Peer Sent Invalid Piece Index For Block Message", detail: None})
        }
        
        let block_offset = try!(self.read_be_u32());
        let block_data_len = get_block_len(payload_len);
        
        if block_data_len as uint <= block_buffer.len() {
            try!(self.read_at_least(block_data_len as uint, block_buffer));
            
            Ok(PeerMessage::BlockReceived(piece_index, block_offset, block_data_len))
        } else {
            let block_data = try!(self.read_exact(block_data_len as uint));
            
            Ok(PeerMessage::BlockReceivedTooBig(piece_index, block_offset, block_data))
        }
    }
}

pub trait PeerWriter {
    fn write_state(&mut self, state: StateChange) -> IoResult<()>;
    
    fn write_have(&mut self, piece: PieceIndex) -> IoResult<()>;
    
    fn write_bitfield(&mut self, bitfield: &[u8]) -> IoResult<()>;
    
    fn write_request(&mut self, piece: PieceIndex, offset: BlockOffset, len: BlockLength) -> IoResult<()>;
    
    fn write_cancel(&mut self, piece: PieceIndex, offset: BlockOffset, len: BlockLength) -> IoResult<()>;
    
    fn write_block(&mut self, piece: PieceIndex, offset: BlockOffset, block_data: &[u8]) -> IoResult<()>;
}

impl<T: Writer> PeerWriter for T {
    fn write_state(&mut self, state: StateChange) -> IoResult<()> {
        try!(self.write_be_u32(STATE_MESSAGE_LEN));

        try!(match state {
            StateChange::Choke        => self.write_u8(CHOKE_ID),
            StateChange::Unchoke      => self.write_u8(UNCHOKE_ID),
            StateChange::Interested   => self.write_u8(INTERESTED_ID),
            StateChange::Uninterested => self.write_u8(UNINTERESTED_ID)
        });
        
        Ok(())
    }
    
    fn write_have(&mut self, piece: PieceIndex) -> IoResult<()> {
        try!(self.write_be_u32(HAVE_MESSAGE_LEN));
        try!(self.write_u8(HAVE_ID));
        try!(self.write_be_u32(piece));
        
        Ok(())
    }
    
    fn write_bitfield(&mut self, bitfield: &[u8]) -> IoResult<()> {
        try!(self.write_be_u32(MESSAGE_ID_LEN + bitfield.len() as u32));
        try!(self.write_u8(BITFIELD_ID));
        try!(self.write(bitfield));
        
        Ok(())
    }
    
    fn write_request(&mut self, piece: PieceIndex, offset: BlockOffset, len: BlockLength) -> IoResult<()> {
        try!(self.write_be_u32(REQUEST_MESSAGE_LEN));
        try!(self.write_u8(REQUEST_ID));
        try!(self.write_be_u32(piece));
        try!(self.write_be_u32(offset));
        try!(self.write_be_u32(len));
        
        Ok(())
    }
    
    fn write_cancel(&mut self, piece: PieceIndex, offset: BlockOffset, len: BlockLength) -> IoResult<()> {
        try!(self.write_be_u32(CANCEL_MESSAGE_LEN));
        try!(self.write_u8(CANCEL_ID));
        try!(self.write_be_u32(piece));
        try!(self.write_be_u32(offset));
        try!(self.write_be_u32(len));
        
        Ok(())
    }
    
    fn write_block(&mut self, piece: PieceIndex, offset: BlockOffset, block_data: &[u8]) -> IoResult<()> {
        try!(self.write_be_u32(BASE_BLOCK_MESSAGE_LEN + block_data.len() as u32));
        try!(self.write_u8(BLOCK_ID));
        try!(self.write_be_u32(piece));
        try!(self.write_be_u32(offset));
        try!(self.write(block_data));
        
        Ok(())
    }
}