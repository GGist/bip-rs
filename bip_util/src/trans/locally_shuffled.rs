use std::ops::Add;
use std::num::Wrapping;

use num::{Bounded, One, Zero};

use crate::trans::{SequentialIds, TransactionIds};

const TRANSACTION_ID_PREALLOC_LEN: usize = 2048;

/// Provides transaction ids that are locally shuffled.
///
/// For all intents and purposes, this class is used to generate
/// transaction ids that need to be random but need to be unique
/// over a reasonable span of time.
///
/// We do this by composing the sequential ids and taking x number
/// of ids into a buffer. We then randomly shuffle that buffer
/// and hand out ids in order. When the buffer is exhausted we repeat.
/// This allows us to uphold the uniqueness property for any large
/// transaction type (such as u64) but also works with smaller types.
pub struct LocallyShuffledIds<T> {
    sequential: SequentialIds<T>,
    stored_ids: Vec<T>
}

impl<T> LocallyShuffledIds<T>
    where T: One + Zero + Clone + Eq + Bounded + Default,
          Wrapping<T>: Add<Wrapping<T>, Output = Wrapping<T>> {
    /// Create a new LocallyShuffledIds struct.
    pub fn new() -> LocallyShuffledIds<T> {
        LocallyShuffledIds::start_at(T::zero())
    }

    /// Create a new LocallyShuffledIds struct at the starting value.
    pub fn start_at(start: T) -> LocallyShuffledIds<T> {
        LocallyShuffledIds{ sequential: SequentialIds::start_at(start), stored_ids: Vec::new() }
    }

    /// Refills our stored ids list with new ids and resets our current index.
    fn refill_stored_ids(&mut self) {
        // Clear out our previous block.
        self.stored_ids.clear();

        // Store these values so we can detect when we hit them.
        let max_value = T::max_value();
        let min_value = T::min_value();

        // If we see that we picked up the min value, then we can't rollover after the max value
        // for the current pre allocation chunk otherwise we will have duplicates in the same block.
        let mut contains_min_value = false;
        let mut contains_max_value = false;

        let mut num_ids_generated = 0;
        while num_ids_generated < TRANSACTION_ID_PREALLOC_LEN && (!contains_min_value || !contains_max_value) {
            let next_id = self.sequential.generate();

            // If we haven't seen the min or max values yet, see if the current id is either of them.
            contains_min_value = contains_min_value || next_id == min_value;
            contains_max_value = contains_max_value || next_id == max_value;

            self.stored_ids.push(next_id);
            num_ids_generated += 1;
        }

        crate::fisher_shuffle(&mut self.stored_ids[..]);
    }
}

impl<T> TransactionIds<T> for LocallyShuffledIds<T>
    where T: One + Zero + Clone + Eq + Bounded + Default,
          Wrapping<T>: Add<Wrapping<T>, Output = Wrapping<T>>{
    fn generate(&mut self) -> T {
        self.stored_ids.pop().unwrap_or_else(|| {
            self.refill_stored_ids();

            self.generate()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::LocallyShuffledIds;
    use crate::trans::TransactionIds;

    #[test]
    fn positive_single_prealloc_u8_overflow() {
        let u8_num_values = 2u32.pow(0u8.count_zeros()) as usize;
        let duplicates_to_find = super::TRANSACTION_ID_PREALLOC_LEN / u8_num_values;

        let mut generator = LocallyShuffledIds::<u8>::new();
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

        let mut generator = LocallyShuffledIds::<u8>::new();
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

        let mut generator = LocallyShuffledIds::<u8>::new();
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

        let mut generator = LocallyShuffledIds::<u8>::new();
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
        let mut generator = LocallyShuffledIds::<u32>::new();
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
        let mut generator = LocallyShuffledIds::<u32>::new();
        let mut tid_count = [0u8; (super::TRANSACTION_ID_PREALLOC_LEN * 2)];

        for tid in (0..(super::TRANSACTION_ID_PREALLOC_LEN * 2)).map(|_| generator.generate()) {
            let index = tid as usize;

            tid_count[index] += 1;
        }

        for count in tid_count.iter() {
            assert_eq!(*count, 1);
        }
    }
}
