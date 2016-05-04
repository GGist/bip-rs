#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct TID {
    id: u64
}

struct TIDGenerator {
    generator: TIDGenerator<u64>
}

impl TIDGenerator {
    pub fn new() -> TIDGenerator {
        TIDGenerator{ generator: TIDGenerator::<u64>::new() }
    }
    
    pub fn generate() -> TID {
        TID{ id: self.generator.generate() }
    }
}