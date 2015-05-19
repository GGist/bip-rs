//! Streaming data to and from the remote peer.

use std::old_io::{IoResult, IoError, InvalidInput};
use std::fmt::{Display, Formatter, Result};
use peer::block::{Block};

const CHOKE_ID:        u8 = 0;
const UNCHOKE_ID:      u8 = 1;
const INTERESTED_ID:   u8 = 2;
const UNINTERESTED_ID: u8 = 3;
const HAVE_ID:         u8 = 4;
const BITFIELD_ID:     u8 = 5;
const REQUEST_ID:      u8 = 6;
const BLOCK_ID:        u8 = 7;
const CANCEL_ID:       u8 = 8;

const KEEP_ALIVE_MESSAGE_LEN: u32 = 0;

const MESSAGE_ID_LEN: u32 = 1;

const STATE_MESSAGE_LEN:   u32 = MESSAGE_ID_LEN;
const HAVE_MESSAGE_LEN:    u32 = MESSAGE_ID_LEN + 4;
const REQUEST_MESSAGE_LEN: u32 = MESSAGE_ID_LEN + 12;
const CANCEL_MESSAGE_LEN:  u32 = MESSAGE_ID_LEN + 12;

const BASE_BLOCK_MESSAGE_LEN: u32 = MESSAGE_ID_LEN + 8;

pub type PieceIndex  = u32;
pub type BlockOffset = u32;
pub type BlockLength = u32;

/// Represents a state change for one end of a connection.
pub enum StateChange {
    Choke,
    Unchoke,
    Interested,
    Uninterested
}

impl Copy for StateChange { }
impl Display for StateChange {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match *self {
            StateChange::Choke        => f.write_str("Choke"),
            StateChange::Unchoke      => f.write_str("Unchoke"),
            StateChange::Interested   => f.write_str("Interested"),
            StateChange::Uninterested => f.write_str("Uninterested")
        }
    }
}

/// Represents a message received from a remote peer.
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

impl Display for PeerMessage {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match *self {
            PeerMessage::Hidden => 
                f.write_str("Hidden"),
            PeerMessage::StateUpdate(ref change) => 
                f.write_fmt(format_args!("StateUpdate({})", change)),
            PeerMessage::HaveUpdate(index) => 
                f.write_fmt(format_args!("HaveUpdate({})", index)),
            PeerMessage::BitfieldUpdate(..) => 
                f.write_str("BitfieldUpdate(Vec<u8>)"),
            PeerMessage::BlockRequest(piece, offset, len) => 
                f.write_fmt(format_args!("BlockRequest({}, {}, {})", piece, offset, len)),
            PeerMessage::CancelRequest(piece, offset, len) => 
                f.write_fmt(format_args!("CancelRequest({}, {}, {})", piece, offset, len)),
            PeerMessage::BlockReceived(piece, offset, len) => 
                f.write_fmt(format_args!("BlockReceived({}, {}, {})", piece, offset, len)),
            PeerMessage::BlockReceivedTooBig(piece, offset, _) => 
                f.write_fmt(format_args!("BlockReceivedTooBig({}, {}, Vec<u8>)", piece, offset))
        }
    }
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

/// Trait for reading Peer Wire Protocol messages. 
///
/// Unlike the corresponding PeerWriter, the methods defined here should do bounds
/// checking on both message lengths and piece indices in order to combat against
/// buffer overflow attacks.
pub trait PeerReader {
    fn read_message<'a, F>(&mut self, max_pieces: u32, block: &mut F) -> IoResult<PeerMessage>
        where F: FnMut(BlockLength) -> &'a mut Block;
    
    fn read_have(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage>;
    
    fn read_bitfield(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage>;
    
    fn read_request(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage>;
    
    fn read_cancel(&mut self, payload_len: u32, max_pieces: u32) -> IoResult<PeerMessage>;
    
    fn read_block(&mut self, payload_len: u32, max_pieces: u32, block_buffer: &mut [u8]) -> IoResult<PeerMessage>;
}

impl<T: Reader> PeerReader for T {
    fn read_message<'a, F>(&mut self, max_pieces: u32, block: &mut F) ->  IoResult<PeerMessage>
        where F: FnMut(BlockLength) -> &'a mut Block {
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
                let block = block(get_block_len(payload_len));
                let message_action = try!(self.read_block(payload_len, max_pieces, block.as_mut_slice()));
                
                if let PeerMessage::BlockReceived(index, offset, length) = message_action {
                    block.set_active(index, offset, length);
                }
                message_action
            },
            _ => { // Allow Unrecognize Message IDs
                let block = block(payload_len);
                
                if payload_len as usize <= block.as_slice().len() {
                    try!(self.read_at_least(payload_len as usize, block.as_mut_slice()));
                } else {
                    try!(self.read_exact(payload_len as usize));
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
        
        let bytes = try!(self.read_exact(payload_len as usize));
        
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
        if block_data_len as usize <= block_buffer.len() {
            try!(self.read_at_least(block_data_len as usize, block_buffer));
            
            Ok(PeerMessage::BlockReceived(piece_index, block_offset, block_data_len))
        } else {
            let block_data = try!(self.read_exact(block_data_len as usize));
            
            Ok(PeerMessage::BlockReceivedTooBig(piece_index, block_offset, block_data))
        }
    }
}

/// Trait for writing Peer Wire Protocol messages.
///
/// No piece validation will be done here, all pieces passed in are assumed to be
/// within bounds.
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
        try!(self.write_all(bitfield));
        
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
        try!(self.write_all(block_data));
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::old_io::{BufWriter, BufReader, SeekSet};
    use peer::block::{Block};
    use super::{PeerReader, PeerWriter, CHOKE_ID, UNCHOKE_ID, INTERESTED_ID, UNINTERESTED_ID, 
        HAVE_ID, BITFIELD_ID, REQUEST_ID, BLOCK_ID, CANCEL_ID, StateChange, MESSAGE_ID_LEN,
        PeerMessage, STATE_MESSAGE_LEN, HAVE_MESSAGE_LEN, REQUEST_MESSAGE_LEN, 
        BASE_BLOCK_MESSAGE_LEN};
    
    const MESSAGE_LENGTH_LEN: u32 = 4;
    
    #[test]
    fn positive_read_write_message() {
        let mut buffer = [0u8; (MESSAGE_LENGTH_LEN + STATE_MESSAGE_LEN) as usize];
        {
            let mut buf_writer = BufWriter::new(buffer.as_mut_slice());
            buf_writer.write_state(StateChange::Choke).unwrap();
        }
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Write
        if buf_reader.read_be_u32().unwrap() != STATE_MESSAGE_LEN ||
           buf_reader.read_u8().unwrap() != CHOKE_ID ||
           !buf_reader.eof() {
            panic!("Write Failed For Single Message")
        }
        
        // Verify Read
        buf_reader.seek(0, SeekSet).unwrap();
        let mut block = Block::with_capacity(0);
        match buf_reader.read_message(0, &mut |_: u32| &mut block) {
            Ok(PeerMessage::StateUpdate(change)) => {
                match change {
                    StateChange::Choke => (),
                    _ => panic!("Read/Write Failed For Single Message")
                }
            },
            e => panic!("Read Failed For Single Message: {}", e.unwrap())
        }
    }
    
        #[test]
    fn positive_read_write_messages() {
        let mut buffer = [0u8; (2 * MESSAGE_LENGTH_LEN + STATE_MESSAGE_LEN + REQUEST_MESSAGE_LEN) as usize];
        let (piece, offset, len) = (0u32, 100u32, 50u32);
        {
            let mut buf_writer = BufWriter::new(buffer.as_mut_slice());
            buf_writer.write_state(StateChange::Interested).unwrap();
            buf_writer.write_request(piece, offset, len).unwrap();
        }
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Write
        if buf_reader.read_be_u32().unwrap() != STATE_MESSAGE_LEN ||
           buf_reader.read_u8().unwrap() != INTERESTED_ID ||
           buf_reader.read_be_u32().unwrap() != REQUEST_MESSAGE_LEN ||
           buf_reader.read_u8().unwrap() != REQUEST_ID ||
           buf_reader.read_be_u32().unwrap() != piece ||
           buf_reader.read_be_u32().unwrap() != offset ||
           buf_reader.read_be_u32().unwrap() != len ||
           !buf_reader.eof() {
            panic!("Write Failed For Multi Message")
        }
        
        // Verify Read
        buf_reader.seek(0, SeekSet).unwrap();
        let mut block = Block::with_capacity(0);
        match buf_reader.read_message(0, &mut |_: u32| &mut block) {
            Ok(PeerMessage::StateUpdate(change)) => {
                match change {
                    StateChange::Interested => (),
                    _ => panic!("Read Failed For Multi Message")
                }
            },
            e => panic!("Read Failed For Multi Message: {}", e.unwrap())
        };
        match buf_reader.read_message(1, &mut |_: u32| &mut block) {
            Ok(PeerMessage::BlockRequest(ret_piece, ret_offset, ret_len)) => {
                if ret_piece != piece || ret_offset != offset || ret_len != len {
                    panic!("Read Failed For Multi Message");
                }
            },
            e => panic!("Read Failed For Multi Message: {}", e.unwrap())
        };
    }
    
    #[test]
    fn positive_read_message_keep_alive() {
        let buffer = [0u8, 0u8, 0u8, 0u8]; // Equal To A 32 Bit 0
        let mut buf_reader = BufReader::new(buffer.as_slice());
        let mut block = Block::with_capacity(0);
        
        match buf_reader.read_message(0, &mut |_: u32| &mut block) {
            Ok(PeerMessage::Hidden) => (),
            e => panic!("Read Failed For Keep Alive Message: {}", e.unwrap())
        };
    }
    
    #[test]
    fn positive_read_unkown_message() {
        let mut buffer = [0u8; 4 + 1 + 100];
        {
            let mut buf_writer = BufWriter::new(buffer.as_mut_slice());
            buf_writer.write_be_u32(101).unwrap();
            buf_writer.write_u8(255).unwrap(); // Some Invalid Action Id
        }
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Force reader to discard contents (don't write to block)
        let mut block = Block::with_capacity(0); 
        match buf_reader.read_message(0, &mut |_: u32| &mut block) {
            Ok(PeerMessage::Hidden) => (),
            e => panic!("Read Failed For Unknown Message: {}", e.unwrap())
        };
    }
    
    // Used To Test All State Message Reads And Writes
    fn read_write_state(state_change: StateChange, state_id: u8) {
        let mut buffer = [0u8; (MESSAGE_LENGTH_LEN + STATE_MESSAGE_LEN) as usize];
        {
            let mut buf_writer = BufWriter::new(buffer.as_mut_slice());
            buf_writer.write_state(state_change).unwrap();
        }
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Write
        if buf_reader.read_be_u32().unwrap() != STATE_MESSAGE_LEN ||
           buf_reader.read_u8().unwrap() != state_id ||
           !buf_reader.eof() {
            panic!("Write Failed For {} Message", state_change)
        }
        
        // Verify Read
        buf_reader.seek(0, SeekSet).unwrap();
        let mut block = Block::with_capacity(0);
        match buf_reader.read_message(0, &mut |_: u32| &mut block) {
            Ok(PeerMessage::StateUpdate(change)) => {
                // Make Sure Enum Passed In Matches Enum Passed Out
                match (change, state_change) {
                    (StateChange::Choke, StateChange::Choke) => (),
                    (StateChange::Unchoke, StateChange::Unchoke) => (),
                    (StateChange::Interested, StateChange::Interested) => (),
                    (StateChange::Uninterested, StateChange::Uninterested) => (),
                    _ => panic!("Read Failed For {} Message", state_change)
                }
            },
            e => panic!("Read Failed For {} Message: {}", state_change, e.unwrap())
        };
    }
    
    #[test]
    fn positive_read_write_choke() {
        read_write_state(StateChange::Choke, CHOKE_ID);
    }
    
    #[test]
    fn positive_read_write_unchoke() {
        read_write_state(StateChange::Unchoke, UNCHOKE_ID);
    }
    
    #[test]
    fn positive_read_write_interested() {
        read_write_state(StateChange::Interested, INTERESTED_ID);
    }
    
    #[test]
    fn positive_read_write_uninterested() {
        read_write_state(StateChange::Uninterested, UNINTERESTED_ID);
    }
    
    fn read_write_have(piece: u32, max_pieces: u32) {
        let mut buffer = [0u8; (MESSAGE_LENGTH_LEN + HAVE_MESSAGE_LEN) as usize];
        {
            let mut buf_writer = BufWriter::new(buffer.as_mut_slice());
            buf_writer.write_have(piece).unwrap();
        }
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Write
        if buf_reader.read_be_u32().unwrap() != HAVE_MESSAGE_LEN ||
           buf_reader.read_u8().unwrap() != HAVE_ID ||
           buf_reader.read_be_u32().unwrap() != piece ||
           !buf_reader.eof() {
            panic!("Write Failed For Have Message")
        }
        
        // Verify Read
        buf_reader.seek((MESSAGE_LENGTH_LEN + MESSAGE_ID_LEN) as i64, SeekSet).unwrap();
        match buf_reader.read_have(HAVE_MESSAGE_LEN - MESSAGE_ID_LEN, max_pieces) {
            Ok(PeerMessage::HaveUpdate(ret_piece)) if ret_piece == piece => (),
            e => panic!("Read Failed For Have Message: {}", e.unwrap())
        };
    }
    
    #[test]
    fn positive_read_write_have() {
        // First Piece
        read_write_have(0, 100);
        // Last Piece
        read_write_have(99, 100);
    }
    
    #[test]
    #[should_fail]
    fn negative_read_write() {
        // Out Of Bounds Piece
        read_write_have(100, 0);
        // Out Of Bounds (Off By 1) Piece
        read_write_have(100, 100);
        // Out Of Bounds (Off By 2) Piece
        read_write_have(101, 100);
    }
    
    fn read_write_bitfield(bytes: &[u8], max_pieces: u32) {
        let buffer_length: usize = (MESSAGE_LENGTH_LEN + MESSAGE_ID_LEN) as usize + bytes.len();
        let mut buffer = Vec::with_capacity(buffer_length);
        
        unsafe{ buffer.set_len(buffer_length); }
        {
            let mut buf_writer = BufWriter::new(buffer.as_mut_slice());
            buf_writer.write_bitfield(bytes).unwrap();
        }
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Write
        if buf_reader.read_be_u32().unwrap() != MESSAGE_ID_LEN + bytes.len() as u32 ||
           buf_reader.read_u8().unwrap() != BITFIELD_ID {
            panic!("Write Failed For Bitfield Message")
        }
        for i in bytes.iter() {
            if *i != buf_reader.read_u8().unwrap() { 
                panic!("Write Failed For Bitfield Message (Payload)")
            }
        }
        if !buf_reader.eof() {
            panic!("Write Failed For Bitfield Message (Extra Bytes)")
        }

        // Verify Read
        buf_reader.seek((MESSAGE_LENGTH_LEN + MESSAGE_ID_LEN) as i64, SeekSet).unwrap();
        match buf_reader.read_bitfield(bytes.len() as u32, max_pieces) {
            Ok(PeerMessage::BitfieldUpdate(ret_bytes)) => {
                for (a, b) in ret_bytes.iter().zip(bytes.iter()) {
                    if a != b {
                        panic!("Write Failed For Bitfield Message (Payload)")
                    }
                }
            },
            e => panic!("Read Failed For Bitfield Message: {}", e.unwrap())
        };
    }
    
    #[test]
    fn positive_read_write_bitfield() {
        // Full Bitfield
        read_write_bitfield([0xEE, 0xA8, 0xBC, 0x44, 0x23, 0x00].as_slice(), 6 * 8);
        // Full Bitfield With 1 Valid Bit In Last Byte
        read_write_bitfield([0xEE, 0xA8, 0xBC, 0x44, 0x23, 0x00].as_slice(), 6 * 8 - 7);
        // Partial Bitfield
        read_write_bitfield([0xEE, 0xA8, 0xBC, 0x44, 0x23, 0x00].as_slice(), 10 * 8);
        // Partial Bitfield With 1 Valid Bit In Last Byte
        read_write_bitfield([0xEE, 0xA8, 0xBC, 0x44, 0x23, 0x00].as_slice(), 10 * 8 - 7);
    }
    
    #[test]
    #[should_fail]
    fn negative_read_write_bitfield() {
        // Extra Byte
        read_write_bitfield([0xEE, 0xA8, 0xBC, 0x44, 0x23, 0x00].as_slice(), 5 * 8);
        // Extra Bytes
        read_write_bitfield([0xEE, 0xA8, 0xBC, 0x44, 0x23, 0x00].as_slice(), 0 * 8);
    }
    
    // Used For Request And Cancel Messages
    fn write_request_or_cancel_message(message_id: u8, piece: u32, offset: u32, len: u32) -> [u8; (MESSAGE_LENGTH_LEN + REQUEST_MESSAGE_LEN) as usize] {
        let mut buffer = [0u8; (MESSAGE_LENGTH_LEN + REQUEST_MESSAGE_LEN) as usize];
        {
            let mut buf_writer = BufWriter::new(buffer.as_mut_slice());
            
            if message_id == REQUEST_ID {
                buf_writer.write_request(piece, offset, len).unwrap();
            } else if message_id == CANCEL_ID {
                buf_writer.write_cancel(piece, offset, len).unwrap();
            } else {
                panic!("Function Cannot Check Message With ID {}", message_id)
            }
        }
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Write
        if buf_reader.read_be_u32().unwrap() != REQUEST_MESSAGE_LEN ||
           buf_reader.read_u8().unwrap() != message_id ||
           buf_reader.read_be_u32().unwrap() != piece ||
           buf_reader.read_be_u32().unwrap() != offset ||
           buf_reader.read_be_u32().unwrap() != len ||
           !buf_reader.eof() {
            panic!("Write Failed For Message")
        }
        
        buffer
    }
    
    #[test]
    fn positive_read_write_request() {
        // Verify Write And Get Buffer
        let (piece, offset, len) = (0, 500, 500);
        let buffer = write_request_or_cancel_message(REQUEST_ID, piece, offset, len);
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Read
        buf_reader.seek((MESSAGE_LENGTH_LEN + MESSAGE_ID_LEN) as i64, SeekSet).unwrap();
        match buf_reader.read_request(REQUEST_MESSAGE_LEN - MESSAGE_ID_LEN, piece + 500) {
            Ok(PeerMessage::BlockRequest(ret_piece, ret_offset, ret_len)) => {
                if piece != ret_piece || offset != ret_offset || len != ret_len {
                    panic!("Read Failed For Request Message")
                }
            },
            e => panic!("Read Failed For Request Message: {}", e.unwrap())
        };
    }
    
    #[test]
    fn positive_read_write_cancel() {
        // Verify Write And Get Buffer
        let (piece, offset, len) = (100, 500, 500);
        let buffer = write_request_or_cancel_message(CANCEL_ID, piece, offset, len);
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Read
        buf_reader.seek((MESSAGE_LENGTH_LEN + MESSAGE_ID_LEN) as i64, SeekSet).unwrap();
        match buf_reader.read_cancel(REQUEST_MESSAGE_LEN - MESSAGE_ID_LEN, piece + 1) {
            Ok(PeerMessage::CancelRequest(ret_piece, ret_offset, ret_len)) => {
                if piece != ret_piece || offset != ret_offset || len != ret_len {
                    panic!("Read Failed For Cancel Message")
                }
            },
            e => panic!("Read Failed For Cancel Message: {}", e.unwrap())
        };
    }
    
    #[test]
    fn positive_read_write_block() {
        const BLOCK_LEN: u32 = 4;
        
        let (piece, offset) = (1, 20);
        let payload = [0xFE, 0x80, 0x92, 0xBA];
        let mut buffer = [0u8; (MESSAGE_LENGTH_LEN + BASE_BLOCK_MESSAGE_LEN + BLOCK_LEN) as usize];
        {
            let mut buf_writer = BufWriter::new(buffer.as_mut_slice());
            buf_writer.write_block(piece, offset, payload.as_slice()).unwrap();
        }
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Write
        if buf_reader.read_be_u32().unwrap() != BASE_BLOCK_MESSAGE_LEN + BLOCK_LEN ||
           buf_reader.read_u8().unwrap() != BLOCK_ID ||
           buf_reader.read_be_u32().unwrap() != piece ||
           buf_reader.read_be_u32().unwrap() != offset ||
           buf_reader.read_u8().unwrap() != payload[0] || buf_reader.read_u8().unwrap() != payload[1] ||
           buf_reader.read_u8().unwrap() != payload[2] || buf_reader.read_u8().unwrap() != payload[3] {
            panic!("Write Failed For Block Message")
        }
        
        // Verify Read
        let mut block = Block::with_capacity(BLOCK_LEN);
        buf_reader.seek((MESSAGE_LENGTH_LEN + MESSAGE_ID_LEN) as i64, SeekSet).unwrap();
        match buf_reader.read_block(BASE_BLOCK_MESSAGE_LEN + BLOCK_LEN - MESSAGE_ID_LEN, piece + 1, block.as_mut_slice()) {
            Ok(PeerMessage::BlockReceived(ret_piece, ret_offset, ret_len)) => {
                let ret_block = block.as_slice();
                if ret_piece != piece || ret_offset != offset || ret_len != BLOCK_LEN {
                    panic!("Read Failed For Block Message (Bad Payload Info)")
                } else if ret_block[0] != payload[0] || ret_block[1] != payload[1] || 
                          ret_block[2] != payload[2] || ret_block[3] != payload[3] {
                    panic!("Read Failed For Block Message (Bad Payload)")
                }
            },
            e => panic!("Read Failed For Block Message: {}", e.unwrap())
        }
    }
    
    #[test]
    fn positive_read_block_big() {
        const BLOCK_LEN: u32 = 5;
        
        let (piece, offset) = (1, 20);
        let payload = [0xFE, 0x80, 0x92, 0xBA, 0x00];
        let mut buffer = [0u8; (MESSAGE_LENGTH_LEN + BASE_BLOCK_MESSAGE_LEN + BLOCK_LEN) as usize];
        {
            let mut buf_writer = BufWriter::new(buffer.as_mut_slice());
            buf_writer.write_block(piece, offset, payload.as_slice()).unwrap();
        }
        let mut buf_reader = BufReader::new(buffer.as_slice());
        
        // Verify Write
        if buf_reader.read_be_u32().unwrap() != BASE_BLOCK_MESSAGE_LEN + BLOCK_LEN ||
           buf_reader.read_u8().unwrap() != BLOCK_ID ||
           buf_reader.read_be_u32().unwrap() != piece ||
           buf_reader.read_be_u32().unwrap() != offset ||
           buf_reader.read_u8().unwrap() != payload[0] || buf_reader.read_u8().unwrap() != payload[1] ||
           buf_reader.read_u8().unwrap() != payload[2] || buf_reader.read_u8().unwrap() != payload[3] {
            panic!("Write Failed For Block Big Message")
        }
        
        // Verify Read
        let mut block = Block::with_capacity(BLOCK_LEN - 1);
        buf_reader.seek((MESSAGE_LENGTH_LEN + MESSAGE_ID_LEN) as i64, SeekSet).unwrap();
        match buf_reader.read_block(BASE_BLOCK_MESSAGE_LEN + BLOCK_LEN - MESSAGE_ID_LEN, piece + 1, block.as_mut_slice()) {
            Ok(PeerMessage::BlockReceivedTooBig(ret_piece, ret_offset, ret_data)) => {
                if ret_piece != piece || ret_offset != offset || ret_data.len() != BLOCK_LEN as usize {
                    panic!("Read Failed For Block Message (Bad Payload Info)")
                } else if ret_data[0] != payload[0] || ret_data[1] != payload[1] || 
                          ret_data[2] != payload[2] || ret_data[3] != payload[3] || 
                          ret_data[4] != payload[4] {
                    panic!("Read Failed For Block Big Message (Bad Payload)")
                }
            },
            e => panic!("Read Failed For Block Big Message: {}", e.unwrap())
        }
    }
}