// We don't really use PingRequests for our current algorithms, but that may change in the future!
#![allow(unused)]

use bip_bencode::{Bencode, BencodeConvert, Dictionary};
use bip_util::bt::NodeId;

use message;
use message::request::{self, RequestValidate};
use error::DhtResult;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PingRequest<'a> {
    trans_id: &'a [u8],
    node_id: NodeId,
}

impl<'a> PingRequest<'a> {
    pub fn new(trans_id: &'a [u8], node_id: NodeId) -> PingRequest<'a> {
        PingRequest {
            trans_id: trans_id,
            node_id: node_id,
        }
    }

    pub fn from_parts(rqst_root: &Dictionary<'a, Bencode<'a>>,
                      trans_id: &'a [u8])
                      -> DhtResult<PingRequest<'a>> {
        let validate = RequestValidate::new(trans_id);

        let node_id_bytes =
            try!(validate.lookup_and_convert_bytes(rqst_root, message::NODE_ID_KEY));
        let node_id = try!(validate.validate_node_id(node_id_bytes));

        Ok(PingRequest::new(trans_id, node_id))
    }

    pub fn transaction_id(&self) -> &'a [u8] {
        self.trans_id
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn encode(&self) -> Vec<u8> {
        (ben_map!{
            //message::CLIENT_TYPE_KEY => ben_bytes!(dht::CLIENT_IDENTIFICATION),
            message::TRANSACTION_ID_KEY => ben_bytes!(self.trans_id),
            message::MESSAGE_TYPE_KEY => ben_bytes!(message::REQUEST_TYPE_KEY),
            message::REQUEST_TYPE_KEY => ben_bytes!(request::PING_TYPE_KEY),
            request::REQUEST_ARGS_KEY => ben_map!{
                message::NODE_ID_KEY => ben_bytes!(self.node_id.as_ref())
            }
        })
            .encode()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PingResponse<'a> {
    trans_id: &'a [u8],
    node_id: NodeId,
}

/// Reuse functionality of ping request since the structures are identical.
impl<'a> PingResponse<'a> {
    pub fn new(trans_id: &'a [u8], node_id: NodeId) -> PingResponse<'a> {
        PingResponse {
            trans_id: trans_id,
            node_id: node_id,
        }
    }

    pub fn from_parts(rsp_root: &Dictionary<'a, Bencode<'a>>,
                      trans_id: &'a [u8])
                      -> DhtResult<PingResponse<'a>> {
        let request = try!(PingRequest::from_parts(rsp_root, trans_id));

        Ok(PingResponse::new(request.trans_id, request.node_id))
    }

    pub fn transaction_id(&self) -> &'a [u8] {
        self.trans_id
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn encode(&self) -> Vec<u8> {
        (ben_map!{
            //message::CLIENT_TYPE_KEY => ben_bytes!(dht::CLIENT_IDENTIFICATION),
            message::TRANSACTION_ID_KEY => ben_bytes!(self.trans_id),
            message::MESSAGE_TYPE_KEY => ben_bytes!(message::RESPONSE_TYPE_KEY),
            message::RESPONSE_TYPE_KEY => ben_map!{
                message::NODE_ID_KEY => ben_bytes!(self.node_id.as_ref())
            }
        })
            .encode()
    }
}
