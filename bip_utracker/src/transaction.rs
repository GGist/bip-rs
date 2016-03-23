use std::u32::{self};

use bip_util::{self};

const MAX_TRANSACTION_ID: u32 = u32::MAX;

// NEEDS TO FIT IN A u32
const TRANSACTION_ID_PREALLOC_LEN: usize = 2048;

pub struct TIDGenerator {
    next_alloc:      u32,
    curr_index:      usize,
    transaction_ids: [u32; TRANSACTION_ID_PREALLOC_LEN]
}

impl TIDGenerator {
    pub fn new() -> TIDGenerator {
        let (next_alloc, mut transaction_ids) = generate_tids(0);
        
        bip_util::fisher_shuffle(&mut transaction_ids);
        
        TIDGenerator{ next_alloc: next_alloc, curr_index: 0, transaction_ids: transaction_ids }
    }
    
    pub fn generate(&mut self) -> u32 {
        let opt_transaction_id = self.transaction_ids.get(self.curr_index).map(|t| *t);
        
        if let Some(transaction_id) = opt_transaction_id {
            self.curr_index += 1;
            
            transaction_id
        } else {
            let (next_alloc, mut transaction_ids) = generate_tids(self.next_alloc);
            
            bip_util::fisher_shuffle(&mut transaction_ids);
            
            self.next_alloc = next_alloc;
            self.transaction_ids = transaction_ids;
            self.curr_index = 0;
            
            self.generate()
        }
    }
}

fn generate_tids(next_alloc: u32) -> (u32, [u32; TRANSACTION_ID_PREALLOC_LEN]) {
    // Check if we need to wrap
    let (next_alloc_start, next_alloc_end) = if next_alloc == MAX_TRANSACTION_ID {
        (0, TRANSACTION_ID_PREALLOC_LEN as u32)
    } else {
        (next_alloc, next_alloc + TRANSACTION_ID_PREALLOC_LEN as u32)
    };
    let mut transaction_ids = [0u32; TRANSACTION_ID_PREALLOC_LEN];
    
    for (index, transaction_id) in (next_alloc_start..next_alloc_end).enumerate() {
        transaction_ids[index] = transaction_id;
    }
    
    (next_alloc_end, transaction_ids)
}