use bip_bencode::{Bencode, BencodeConvert, Dictionary};

use error::{DhtResult, DhtError, DhtErrorKind};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PutDataRequest<'a> {
    trans_id: &'a [u8]
}

impl<'a> PutDataRequest<'a> {
    pub fn new(rqst_root: &Dictionary<'a, Bencode<'a>>, trans_id: &'a [u8]) -> DhtResult<PutDataRequest<'a>> {
        unimplemented!();
    }
    
    pub fn transaction_id(&self) -> &'a [u8] {
        self.trans_id
    }
}