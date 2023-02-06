use std::num::Wrapping;
use std::ops::Add;

use num::{One, Zero};

use crate::trans::TransactionIds;

/// Generates sequentially unique ids and wraps when overflow occurs.
pub struct SequentialIds<T> {
    next_id: T,
}

impl<T> SequentialIds<T>
where
    T: Zero,
{
    /// Create a new SequentialIds struct.
    pub fn new() -> SequentialIds<T> {
        SequentialIds::start_at(T::zero())
    }

    /// Create a new SequentialIds struct at the starting value.
    pub fn start_at(start: T) -> SequentialIds<T> {
        SequentialIds { next_id: start }
    }
}

impl<T> TransactionIds<T> for SequentialIds<T>
where
    T: One + Clone,
    Wrapping<T>: Add<Wrapping<T>, Output = Wrapping<T>>,
{
    fn generate(&mut self) -> T {
        let curr_id = self.next_id.clone();
        self.next_id = (Wrapping(self.next_id.clone()) + Wrapping(T::one())).0;

        curr_id
    }
}

#[cfg(test)]
mod tests {
    use super::SequentialIds;
    use crate::trans::TransactionIds;

    #[test]
    fn positive_sequentail_zero_initial() {
        let mut sequential_ids = SequentialIds::<u8>::new();

        assert_eq!(0, sequential_ids.generate());
    }

    #[test]
    fn positive_sequential_u8_overflow() {
        let mut sequential_ids = SequentialIds::<u8>::new();

        let init_value = sequential_ids.generate();
        for _ in 0..255 {
            sequential_ids.generate();
        }

        assert_eq!(init_value, sequential_ids.generate());
    }

    #[test]
    fn positive_sequentail_start_at() {
        let mut sequential_ids = SequentialIds::<u8>::start_at(55);

        assert_eq!(55, sequential_ids.generate());
    }
}
