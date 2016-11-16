mod sequential;
mod locally_shuffled;

trait TransactionIds<T> {
    fn generate(&mut self) -> T;
}

pub use self::sequential::SequentialIds;
pub use self::locally_shuffled::LocallyShuffledIds;