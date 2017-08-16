pub enum PeerExtensionProtocolMessage<P> where P: PeerProtocol {
    LtMetadata(LtMetadataMesage),
    UtPex(UtPexMessage),
    Custom(P::PeerMessage)
}

// ----------------------------------------------------------------------------//

pub struct LtMetadataMessage {
    Request(LtMetadataRequestMessage),
    Data(LtMetadataDataMessage),
    Reject(LtMetadataRejectMessage)
}


pub struct LtMetadataRequestMessage {
    piece: i64
}

impl LtMetadataRequestMessage {
    pub fn new(piece: i64) -> LtMetadataRequestMessage {
        LtMetadataRequestMessage{ piece: piece }
    }
}

pub struct LtMetadataDataMessage {
    piece:      i64,
    total_size: i64
}



pub struct LtMetadataRejectMessage {
    piece: i64
}

// ----------------------------------------------------------------------------//