use std::ops::{Add};
use std::num::{Wrapping};

use num::{NumCast, Bounded, One};

const TRANSACTION_ID_PREALLOC_LEN: usize = 2048;

/// Generates blocks of transaction ids where each block is shuffled before being used.
///
/// Works on all primitive numbers as well as non pot numbers. For numbers whose max value
/// is smaller than the pre allocation length, it is possible for duplicates of those numbers
/// to appear in the same block and, by extension, one after the other back to back.
pub struct TIDGenerator<T> {
    next_alloc: T,
    curr_index: usize,
    trans_ids:  [T; TRANSACTION_ID_PREALLOC_LEN]
}

impl<T> TIDGenerator<T> where T: Copy + Bounded + NumCast + One + Eq, Wrapping<T>: Add<Wrapping<T>, Output=Wrapping<T>> {
    /// Create a new TIDGenerator.
    pub fn new() -> TIDGenerator<T> {
        let (next_alloc, mut transaction_ids) = generate_tids(T::min_value());
        
        ::fisher_shuffle(&mut transaction_ids);
        
        TIDGenerator{ next_alloc: next_alloc, curr_index: 0, trans_ids: transaction_ids }
    }
    
    /// Grab the next transaction id that was generated.
    pub fn generate(&mut self) -> T {
        let opt_transaction_id = self.trans_ids.get(self.curr_index).map(|t| *t);
        
        if let Some(transaction_id) = opt_transaction_id {
            self.curr_index += 1;
            
            transaction_id
        } else {
            let (next_alloc, mut transaction_ids) = generate_tids(self.next_alloc);
            
            ::fisher_shuffle(&mut transaction_ids);
            
            self.next_alloc = next_alloc;
            self.trans_ids = transaction_ids;
            self.curr_index = 0;
            
            self.generate()
        }
    }
}

/// Generate a new block of transaction ids starting from the current allocation marker.
fn generate_tids<T: Copy>(curr_alloc: T) -> (T, [T; TRANSACTION_ID_PREALLOC_LEN])
    where T: Copy + Bounded + NumCast + One + Eq, Wrapping<T>: Add<Wrapping<T>, Output=Wrapping<T>> {
    let max_trans_id = T::max_value();

    let (next_alloc_start, next_alloc_end) = match NumCast::from(TRANSACTION_ID_PREALLOC_LEN) {
        Some(alloc_len) => {
            if curr_alloc == max_trans_id {
                (T::min_value(), alloc_len)
            } else {
                (curr_alloc, (Wrapping(curr_alloc) + Wrapping(alloc_len)).0)
            }
        },
        None => {
            // If the cast failed, we presume our type has a smaller max value than the pre
            // allocation amount. In that case, just repeat the values and return the same
            // next alloc since we are really just wrapping around multiple time to fill in
            // the transaction ids (not ideal but if the client wants this...).
            (curr_alloc, curr_alloc)
        }
    };
    let mut transaction_ids = [T::one(); TRANSACTION_ID_PREALLOC_LEN];
    
    let mut next_tid = next_alloc_start;
    for slot in transaction_ids.iter_mut() {
        *slot = next_tid;
        
        next_tid = (Wrapping(next_tid) + Wrapping(T::one())).0;
    }
    
    (next_alloc_end, transaction_ids)
}

#[cfg(test)]
mod tests {
    use super::{TIDGenerator};
    
    #[test]
    fn positive_single_prealloc_u8_overflow() {
        let u8_num_values = 2u32.pow(0u8.count_zeros()) as usize;
        let duplicates_to_find = super::TRANSACTION_ID_PREALLOC_LEN / u8_num_values;
    
        let mut generator = TIDGenerator::<u8>::new();
        let mut tid_count = vec![0u8; u8_num_values];
        
        // Loop around the pre allocation length once
        for tid in (0..super::TRANSACTION_ID_PREALLOC_LEN).map(|_| generator.generate()) {
            let index = tid as usize;
        
            tid_count[index] += 1;
        }
        
        for count in tid_count.iter() {
            assert_eq!(*count, duplicates_to_find as u8);
        }
    }
    
    #[test]
    fn positive_multiple_prealloc_u8_overflow() {
        let u8_num_values = 2u32.pow(0u8.count_zeros()) as usize;
        let duplicates_to_find = (super::TRANSACTION_ID_PREALLOC_LEN / u8_num_values) * 2;
    
        let mut generator = TIDGenerator::<u8>::new();
        let mut tid_count = vec![0u8; u8_num_values];
        
        // Loop around the pre allocation length once
        for tid in (0..(super::TRANSACTION_ID_PREALLOC_LEN * 2)).map(|_| generator.generate()) {
            let index = tid as usize;
            
            tid_count[index] += 1;
        }
        
        for count in tid_count.iter() {
            assert_eq!(*count, duplicates_to_find as u8);
        }
    }
    
    #[test]
    fn positive_single_prealloc_i8_overflow() {
        let i8_num_values = 2u32.pow(0i8.count_zeros()) as usize;
        let duplicates_to_find = super::TRANSACTION_ID_PREALLOC_LEN / i8_num_values;
    
        let mut generator = TIDGenerator::<u8>::new();
        let mut tid_count = vec![0i8; i8_num_values];
        
        // Loop around the pre allocation length once
        for tid in (0..super::TRANSACTION_ID_PREALLOC_LEN).map(|_| generator.generate()) {
            let index = tid as usize;
        
            tid_count[index] += 1;
        }
        
        for count in tid_count.iter() {
            assert_eq!(*count, duplicates_to_find as i8);
        }
    }
    
    #[test]
    fn positive_multiple_prealloc_i8_overflow() {
        let i8_num_values = 2u32.pow(0i8.count_zeros()) as usize;
        let duplicates_to_find = (super::TRANSACTION_ID_PREALLOC_LEN / i8_num_values) * 2;
    
        let mut generator = TIDGenerator::<u8>::new();
        let mut tid_count = vec![0i8; i8_num_values];
        
        // Loop around the pre allocation length once
        for tid in (0..(super::TRANSACTION_ID_PREALLOC_LEN * 2)).map(|_| generator.generate()) {
            let index = tid as usize;
        
            tid_count[index] += 1;
        }
        
        for count in tid_count.iter() {
            assert_eq!(*count, duplicates_to_find as i8);
        }
    }
    
    #[test]
    fn positive_single_prealloc_u32_no_overflow() {
        let mut generator = TIDGenerator::<u32>::new();
        let mut tid_count = [0u8; super::TRANSACTION_ID_PREALLOC_LEN];
        
        for tid in (0..super::TRANSACTION_ID_PREALLOC_LEN).map(|_| generator.generate()) {
            let index = tid as usize;
            
            tid_count[index] += 1;
        }
        
        for count in tid_count.iter() {
            assert_eq!(*count, 1);
        }
    }
    
    #[test]
    fn positive_multiple_prealloc_u32_no_overflow() {
        let mut generator = TIDGenerator::<u32>::new();
        let mut tid_count = [0u8; (super::TRANSACTION_ID_PREALLOC_LEN * 2)];
        
        for tid in (0..(super::TRANSACTION_ID_PREALLOC_LEN * 2)).map(|_| generator.generate()) {
            let index = tid as usize;
            
            tid_count[index] += 1;
        }
        
        for count in tid_count.iter() {
            assert_eq!(*count, 1);
        }
    }
    
    #[test]
    fn positive_multiple_prealloc_u32_overflow() {
        // Subtract 1 because this isnt the number of u32 values (single bit #32 active)
        let last_block_alloc = u32::max_value() - (super::TRANSACTION_ID_PREALLOC_LEN - 1) as u32;
        
        // Verify the last block
        let (next_alloc, ids) = super::generate_tids(last_block_alloc);
        let mut id_filter = [0u8; super::TRANSACTION_ID_PREALLOC_LEN];
        
        assert_eq!(next_alloc, 0);
        
        for &id in ids.iter() {
            let index = (id - last_block_alloc) as usize;
            
            id_filter[index] += 1;
        }
        assert!(id_filter.iter().all(|&found| found == 1));
        
        
        // Verify the overflow allocation
        let (o_next_alloc, o_ids) = super::generate_tids(next_alloc);
        let mut o_id_filter = [0u8; super::TRANSACTION_ID_PREALLOC_LEN];
        
        assert_eq!(o_next_alloc, super::TRANSACTION_ID_PREALLOC_LEN as u32);
        
        for &id in o_ids.iter() {
            let index = id as usize;
            
            o_id_filter[index] += 1;
        }
        assert!(o_id_filter.iter().all(|&found| found == 1));
    }
}