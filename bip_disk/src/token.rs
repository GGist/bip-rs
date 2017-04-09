use bip_util::trans::{TransactionIds, SequentialIds};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Token {
    id: u64,
}

pub struct TokenGenerator {
    generator: SequentialIds<u64>
}

impl TokenGenerator {
    pub fn new() -> TokenGenerator {
        TokenGenerator { generator: SequentialIds::<u64>::new() }
    }

    pub fn generate(&mut self) -> Token {
        Token { id: self.generator.generate() }
    }
}