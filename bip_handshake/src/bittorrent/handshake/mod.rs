use bip_util::bt::PeerId;

use bittorrent::seed::{InitiateSeed, CompleteSeed, PartialBTSeed, EmptyBTSeed, BTSeed};

pub mod context;
pub mod parse;
pub mod protocol;

pub enum HandshakeSeed {
    Initiate(InitiateSeed),
    Complete(CompleteSeed),
}

#[derive(Copy, Clone)]
enum HandshakeState {
    Initiate(InitiateState, Option<PeerId>),
    Complete(CompleteState),
}

#[derive(Copy, Clone)]
enum InitiateState {
    WriteMessage(PartialBTSeed),
    ReadLength(PartialBTSeed),
    ReadMessage(PartialBTSeed),
    Done(PartialBTSeed),
}

#[derive(Copy, Clone)]
enum CompleteState {
    ReadLength(EmptyBTSeed),
    ReadMessage(EmptyBTSeed),
    WriteMessage(EmptyBTSeed),
    Done(BTSeed),
}
