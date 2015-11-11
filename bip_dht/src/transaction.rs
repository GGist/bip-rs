use rand::{self};

use bip_util::{GenericError, GenericResult};

// TODO: Redesign this module

pub const TRANSACTION_ID_LEN: usize = 4;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct TransactionId {
    id: [u8; TRANSACTION_ID_LEN]
}

impl TransactionId {
    pub fn new() -> TransactionId {
        let mut bytes = [0u8; TRANSACTION_ID_LEN];
        
        for byte in bytes.iter_mut() {
            *byte = rand::random::<u8>();
        }
        
        TransactionId{ id: bytes }
    }
    
    pub fn from_bytes(bytes: &[u8]) -> GenericResult<TransactionId> {
        if bytes.len() != TRANSACTION_ID_LEN {
            Err(GenericError::InvalidLength(TRANSACTION_ID_LEN))
        } else {
            let mut id = [0u8; TRANSACTION_ID_LEN];
            
            for (src, dst) in id.iter_mut().zip(bytes.iter()) {
                *src = *dst;
            }
            
            Ok(TransactionId{ id: id })
        }
    }
    
    pub fn as_bytes(&self) -> &[u8] {
        &self.id
    }
}

impl Into<[u8; TRANSACTION_ID_LEN]> for TransactionId {
    fn into(self) -> [u8; TRANSACTION_ID_LEN] {
        self.id
    }
}

impl From<[u8; TRANSACTION_ID_LEN]> for TransactionId {
    fn from(bytes: [u8; TRANSACTION_ID_LEN]) -> TransactionId {
        TransactionId{ id: bytes }
    }
}