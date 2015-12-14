use bip_util::{self};
use bip_util::convert::{self};

// Transaction IDs are going to be vital for both scalability and performance concerns.
// They allow us to both protect against unsolicited responses as well as dropping those
// messages as soon as possible. We are taking an absurdly large, lazily generate, ringbuffer
// approach to generating transaction ids.

// We are going for a simple, stateless (for the most part) implementation for generating
// the transaction ids. We chose to go this route because 1, we dont want to reuse transaction
// ids used in recent requests that had subsequent responses as well because this would make us
// vulnerable to nodes that we gave that transaction id to, they would know we would be reusing
// it soon. And 2, that makes for an unscalable approach unless we also have a timeout for ids
// that we never received responses for which would lend itself to messy code.

// Instead, we are going to pre-allocate a chunk of ids, shuffle them, and use them until they
// run out, then pre-allocate some more, shuffle them, and use them. When we run out, (which wont
// happen for a VERY long time) we will simply wrap around. Also, we are going to break down the
// transaction id, so our transaction id will be made up of the first 5 bytes which will be the
// action id, this would be something like an individual lookup, a bucket refresh, or a bootstrap.
// Now, each of those actions have a number of messages associated with them, this is where the
// last 3 bytes come in which will be the message id. This allows us to route messages appropriately
// and associate them with some action we are performing right down to a message that the action is
// expecting. The pre-allocation strategy is used both on the action id level as well as the message
// id level.

// To protect against timing attacks, where recently pinged nodes got our transaction id and wish
// to guess other transaction ids in the block that we may have in flight, we will make the pre-allocation
// space fairly large so that our shuffle provides a strong protection from these attacks. In the future,
// we may want to dynamically ban nodes that we feel are guessing our transaction ids.

// IMPORTANT: Allocation markers (not the actual allocated ids) are not shifted so that we can deal with
// overflow by manually checking since I dont want to rely on langauge level overflows and whether they
// cause a panic or not (debug and release should have similar semantics)!

// Together these make up 8 bytes, or, a u64
const TRANSACTION_ID_BYTES: usize = ACTION_ID_BYTES + MESSAGE_ID_BYTES;
const ACTION_ID_BYTES:      usize = 5;
const MESSAGE_ID_BYTES:     usize = 3;

// Maximum exclusive value for an action id
const ACTION_ID_SHIFT: usize = ACTION_ID_BYTES * 8;
const MAX_ACTION_ID:   u64   = 1 << ACTION_ID_SHIFT;

// Maximum exclusive value for a message id
const MESSAGE_ID_SHIFT: usize = MESSAGE_ID_BYTES * 8;
const MAX_MESSAGE_ID:   u64   = 1 << MESSAGE_ID_SHIFT;

// Multiple of two so we can wrap around nicely
#[cfg(not(test))]
const ACTION_ID_PREALLOC_LEN:  usize = 2048;
#[cfg(not(test))]
const MESSAGE_ID_PREALLOC_LEN: usize = 2048;

// Reduce the pre allocation length in tests to speed them up significantly
#[cfg(test)]
const ACTION_ID_PREALLOC_LEN:  usize = 16;
#[cfg(test)]
const MESSAGE_ID_PREALLOC_LEN: usize = 16;

pub struct AIDGenerator {
    // NOT SHIFTED, so that we can wrap around manually!
    next_alloc: u64,
    curr_index: usize,
    action_ids: [u64; ACTION_ID_PREALLOC_LEN]
}

impl AIDGenerator {
    pub fn new() -> AIDGenerator {
        let (next_alloc, mut action_ids) = generate_aids(0);
        
        // Randomize the order of ids
        bip_util::fisher_shuffle(&mut action_ids);
        
        AIDGenerator{ next_alloc: next_alloc, curr_index: 0, action_ids: action_ids }
    }
    
    pub fn generate(&mut self) -> MIDGenerator {
        let opt_action_id = self.action_ids.get(self.curr_index).map(|a| *a);
        
        if let Some(action_id) = opt_action_id {
            self.curr_index += 1;
            
            // Shift the action id to make room for the message id
            MIDGenerator::new(action_id << MESSAGE_ID_SHIFT)
        } else {
            // Get a new block of action ids
            let (next_alloc, mut action_ids) = generate_aids(self.next_alloc);
            
            // Randomize the order of ids
            bip_util::fisher_shuffle(&mut action_ids);
            
            self.next_alloc = next_alloc;
            self.action_ids = action_ids;
            self.curr_index = 0;
            
            self.generate()
        }
    }
}

// (next_alloc, aids)
fn generate_aids(next_alloc: u64) -> (u64, [u64; ACTION_ID_PREALLOC_LEN]) {
    // Check if we need to wrap
    let (next_alloc_start, next_alloc_end) = if next_alloc == MAX_ACTION_ID {
        (0, ACTION_ID_PREALLOC_LEN as u64)
    } else {
        (next_alloc, next_alloc + ACTION_ID_PREALLOC_LEN as u64)
    };
    let mut action_ids = [0u64; ACTION_ID_PREALLOC_LEN];
    
    for (index, action_id) in (next_alloc_start..next_alloc_end).enumerate() {
        action_ids[index] = action_id;
    }
    
    (next_alloc_end, action_ids)
}

//----------------------------------------------------------------------------//

pub struct MIDGenerator {
    // ALREADY SHIFTED, for your convenience :)
    action_id:   u64,
    // NOT SHIFTED, so that we can wrap around manually!
    next_alloc:  u64,
    curr_index:  usize,
    message_ids: [u64; MESSAGE_ID_PREALLOC_LEN]
}

impl MIDGenerator {
    // Accepts an action id that has ALREADY BEEN SHIFTED!
    fn new(action_id: u64) -> MIDGenerator {
        // In order to speed up tests, we will generate the first block lazily.
        MIDGenerator{ action_id: action_id, next_alloc: 0, curr_index: MESSAGE_ID_PREALLOC_LEN,
            message_ids: [0u64; MESSAGE_ID_PREALLOC_LEN] }
    }
    
    pub fn action_id(&self) -> ActionID {
        ActionID::from_transaction_id(self.action_id)
    }
    
    pub fn generate(&mut self) -> TransactionID {
        let opt_message_id = self.message_ids.get(self.curr_index).map(|m| *m);
        
        if let Some(message_id) = opt_message_id {
            self.curr_index += 1;
            
            TransactionID::new(self.action_id | message_id)
        } else {
            // Get a new block of message ids
            let (next_alloc, mut message_ids) = generate_mids(self.next_alloc);
            
            // Randomize the order of ids
            bip_util::fisher_shuffle(&mut message_ids);
            
            self.next_alloc = next_alloc;
            self.message_ids = message_ids;
            self.curr_index = 0;
            
            self.generate()
        }
    }
}

// (next_alloc, mids)
fn generate_mids(next_alloc: u64) -> (u64, [u64; MESSAGE_ID_PREALLOC_LEN]) {
    // Check if we need to wrap
    let (next_alloc_start, next_alloc_end) = if next_alloc == MAX_MESSAGE_ID {
        (0, MESSAGE_ID_PREALLOC_LEN as u64)
    } else {
        (next_alloc, next_alloc + MESSAGE_ID_PREALLOC_LEN as u64)
    };
    let mut message_ids = [0u64; MESSAGE_ID_PREALLOC_LEN];
    
    for (index, message_id) in (next_alloc_start..next_alloc_end).enumerate() {
        message_ids[index] = message_id;
    }
    
    (next_alloc_end, message_ids)
}


//----------------------------------------------------------------------------//

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct TransactionID {
    trans_id:       u64,
    trans_id_bytes: [u8; TRANSACTION_ID_BYTES]
}

impl TransactionID {
    fn new(trans_id: u64) -> TransactionID {
        let trans_id_bytes = convert::eight_bytes_to_array(trans_id);
        
        TransactionID{ trans_id: trans_id, trans_id_bytes: trans_id_bytes }
    }
    
    /// Construct a transaction id from a series of bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<TransactionID> {
        if bytes.len() != TRANSACTION_ID_BYTES {
            return None
        }
        let mut trans_id = 0u64;
        
        for byte in bytes.iter() {
            trans_id <<= 8;
            trans_id |= *byte as u64;
        }
        
        Some(TransactionID::new(trans_id))
    }
    
    pub fn action_id(&self) -> ActionID {
        ActionID::from_transaction_id(self.trans_id)
    }
    
    #[allow(unused)]
    pub fn message_id(&self) -> MessageID {
        MessageID::from_transaction_id(self.trans_id)
    }
}

impl AsRef<[u8]> for TransactionID {
    fn as_ref(&self) -> &[u8] {
        &self.trans_id_bytes
    }
}

//----------------------------------------------------------------------------//

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ActionID {
    action_id: u64
}

impl ActionID {
    fn from_transaction_id(trans_id: u64) -> ActionID {
        // The ACTUAL action id
        let shifted_action_id = trans_id >> MESSAGE_ID_SHIFT;
        
        ActionID{ action_id: shifted_action_id }
    }
}

//----------------------------------------------------------------------------//

#[allow(unused)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct MessageID {
    message_id: u64
}

impl MessageID {
    #[allow(unused)]
    fn from_transaction_id(trans_id: u64) -> MessageID {
        let clear_action_id = MAX_MESSAGE_ID - 1;
        // The ACTUAL message id
        let shifted_message_id = trans_id & clear_action_id;
        
        MessageID{ message_id: shifted_message_id }
    }
}

//----------------------------------------------------------------------------//

#[cfg(test)]
mod tests {
    use std::collections::{HashSet};
    
    use super::{AIDGenerator, TransactionID};
    
    #[test]
    fn positive_tid_from_bytes() {
        let mut aid_generator = AIDGenerator::new();
        let mut mid_generator = aid_generator.generate();
        
        let tid = mid_generator.generate();
        let tid_from_bytes = TransactionID::from_bytes(tid.as_ref()).unwrap();
        
        assert_eq!(tid, tid_from_bytes);
    }
    
    #[test]
    fn positive_unique_aid_blocks() {
        // Go through ten blocks worth of action ids, make sure they are unique
        let mut action_ids = HashSet::new();
        let mut aid_generator = AIDGenerator::new();
        
        for _ in 0..(super::ACTION_ID_PREALLOC_LEN * 10) {
            let action_id = aid_generator.generate().action_id();
            
            assert!(!action_ids.contains(&action_id));
            
            action_ids.insert(action_id);
        }
    }
    
    #[test]
    fn positive_unique_mid_blocks() {
        // Go through ten blocks worth of message ids, make sure they are unique
        let mut message_ids = HashSet::new();
        let mut aid_generator = AIDGenerator::new();
        let mut mid_generator = aid_generator.generate();
        
        for _ in 0..(super::MESSAGE_ID_PREALLOC_LEN * 10) {
            let message_id = mid_generator.generate().message_id();
            
            assert!(!message_ids.contains(&message_id));
            
            message_ids.insert(message_id);
        }
    }
    
    #[test]
    fn positive_unique_tid_blocks() {
        // Go through two blocks of compound ids (transaction ids), make sure they are unique
        let mut transaction_ids = HashSet::new();
        let mut aid_generator = AIDGenerator::new();
        
        for _ in 0..(super::ACTION_ID_PREALLOC_LEN) {
            let mut mid_generator = aid_generator.generate();
            
            for _ in 0..(super::MESSAGE_ID_PREALLOC_LEN) {
                let transaction_id = mid_generator.generate();
                
                assert!(!transaction_ids.contains(&transaction_id));
                
                transaction_ids.insert(transaction_id);
            }
        }
    }
    
    #[test]
    fn positive_overflow_aid_generate() {
        let mut action_ids = HashSet::new();
        let mut aid_generator = AIDGenerator::new();
        
        // Track all action ids in the first block
        for _ in 0..(super::ACTION_ID_PREALLOC_LEN) {
            let action_id = aid_generator.generate().action_id();
            
            assert!(!action_ids.contains(&action_id));
            
            action_ids.insert(action_id);
        }
        
        // Modify private variables to overflow back to first block
        aid_generator.next_alloc = super::MAX_ACTION_ID;
        aid_generator.curr_index = super::ACTION_ID_PREALLOC_LEN;
        
        // Check all action ids in the block (should be first block)
        for _ in 0..(super::ACTION_ID_PREALLOC_LEN) {
            let action_id = aid_generator.generate().action_id();
            
            assert!(action_ids.remove(&action_id));
        }
        
        assert!(action_ids.is_empty());
    }
    
    #[test]
    fn positive_overflow_mid_generate() {
        let mut message_ids = HashSet::new();
        let mut aid_generator = AIDGenerator::new();
        let mut mid_generator = aid_generator.generate();
        
        // Track all message ids in the first block
        for _ in 0..(super::MESSAGE_ID_PREALLOC_LEN) {
            let message_id = mid_generator.generate().message_id();
            
            assert!(!message_ids.contains(&message_id));
            
            message_ids.insert(message_id);
        }
        
        // Modify private variables to overflow back to first block
        mid_generator.next_alloc = super::MAX_MESSAGE_ID;
        mid_generator.curr_index = super::MESSAGE_ID_PREALLOC_LEN;
        
        // Check all message ids in the block (should be first block)
        for _ in 0..(super::MESSAGE_ID_PREALLOC_LEN) {
            let message_id = mid_generator.generate().message_id();
            
            assert!(message_ids.remove(&message_id));
        }
        
        assert!(message_ids.is_empty());
    }
    
    #[test]
    fn positive_overflow_tid_generate() {
        let mut transaction_ids = HashSet::new();
        let mut aid_generator = AIDGenerator::new();
        
        // Track all transaction ids in the first block
        for _ in 0..(super::ACTION_ID_PREALLOC_LEN) {
            let mut mid_generator = aid_generator.generate();
            
            for _ in 0..(super::MESSAGE_ID_PREALLOC_LEN) {
                let transaction_id = mid_generator.generate();
                
                assert!(!transaction_ids.contains(&transaction_id));
                
                transaction_ids.insert(transaction_id);
            }
        }
        
        // Modify private variables to overflow back to first block
        aid_generator.next_alloc = super::MAX_ACTION_ID;
        aid_generator.curr_index = super::ACTION_ID_PREALLOC_LEN;
        
        // Check all transaction ids in the block (should be first block)
        for _ in 0..(super::ACTION_ID_PREALLOC_LEN) {
            let mut mid_generator = aid_generator.generate();
            
            for _ in 0..(super::MESSAGE_ID_PREALLOC_LEN) {
                let transaction_id = mid_generator.generate();
                
                assert!(transaction_ids.remove(&transaction_id));
            }
        }
        
        assert!(transaction_ids.is_empty());
    }
}