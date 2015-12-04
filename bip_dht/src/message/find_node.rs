use bip_bencode::{Bencode, BencodeConvert, Dictionary};
use bip_util::bt::{NodeId};

use message::{self};
use message::compact_info::{CompactNodeInfo};
use message::request::{self, RequestValidate};
use message::response::{ResponseValidate};
use error::{DhtResult};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct FindNodeRequest<'a> {
    trans_id:  &'a [u8],
    node_id:   NodeId,
    target_id: NodeId
}

impl<'a> FindNodeRequest<'a> {
    pub fn new(trans_id: &'a [u8], node_id: NodeId, target_id: NodeId) -> FindNodeRequest<'a> {
        FindNodeRequest{ trans_id: trans_id, node_id: node_id, target_id: target_id }
    }

    /// Create a FindNodeRequest from parts.
    ///
    /// The target_key argument is provided for cases where, due to forward compatibility,
    /// the target key we are interested in could fall under the target key or another key.
    pub fn from_parts(rqst_root: &Dictionary<'a, Bencode<'a>>, trans_id: &'a [u8], target_key: &str)
        -> DhtResult<FindNodeRequest<'a>> {
        let validate = RequestValidate::new(trans_id);
        
        let node_id_bytes = try!(validate.lookup_and_convert_bytes(rqst_root, message::NODE_ID_KEY));
        let node_id = try!(validate.validate_node_id(node_id_bytes));
        
        let target_id_bytes = try!(validate.lookup_and_convert_bytes(rqst_root, target_key));
        let target_id = try!(validate.validate_node_id(target_id_bytes));
        
        Ok(FindNodeRequest::new(trans_id, node_id, target_id))
    }
    
    pub fn transaction_id(&self) -> &'a [u8] {
        self.trans_id
    }
    
    pub fn target_id(&self) -> NodeId {
        self.target_id
    }
    
    pub fn encode(&self) -> Vec<u8> {
        (ben_map!{
            //message::CLIENT_TYPE_KEY => ben_bytes!(dht::CLIENT_IDENTIFICATION),
            message::TRANSACTION_ID_KEY => ben_bytes!(self.trans_id),
            message::MESSAGE_TYPE_KEY => ben_bytes!(message::REQUEST_TYPE_KEY),
            message::REQUEST_TYPE_KEY => ben_bytes!(request::FIND_NODE_TYPE_KEY),
            request::REQUEST_ARGS_KEY => ben_map!{
                message::NODE_ID_KEY => ben_bytes!(self.node_id.as_ref()),
                message::TARGET_ID_KEY => ben_bytes!(self.target_id.as_ref())
            }
        }).encode()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct FindNodeResponse<'a> {
    trans_id: &'a [u8],
    node_id:  NodeId,
    nodes:    CompactNodeInfo<'a>
}

impl<'a> FindNodeResponse<'a> {
    pub fn new(trans_id: &'a [u8], node_id: NodeId, nodes: &'a [u8]) -> DhtResult<FindNodeResponse<'a>> {
        let validate = ResponseValidate::new(trans_id);
        let compact_nodes = try!(validate.validate_nodes(nodes));
        
        Ok(FindNodeResponse{ trans_id: trans_id, node_id: node_id, nodes: compact_nodes })
    }

    pub fn from_parts(rsp_root: &Dictionary<'a, Bencode<'a>>, trans_id: &'a [u8]) -> DhtResult<FindNodeResponse<'a>> {
        let validate = ResponseValidate::new(trans_id);
        
        let node_id_bytes = try!(validate.lookup_and_convert_bytes(rsp_root, message::NODE_ID_KEY));
        let node_id = try!(validate.validate_node_id(node_id_bytes));
        
        let nodes = try!(validate.lookup_and_convert_bytes(rsp_root, message::NODES_KEY));
        
        FindNodeResponse::new(trans_id, node_id, nodes)
    }
    
    pub fn transaction_id(&self) -> &'a [u8] {
        self.trans_id
    }
    
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
    
    pub fn nodes(&self) -> CompactNodeInfo<'a> {
        self.nodes
    }
    
    pub fn encode(&self) -> Vec<u8> {
        (ben_map!{
            //message::CLIENT_TYPE_KEY => ben_bytes!(dht::CLIENT_IDENTIFICATION),
            message::TRANSACTION_ID_KEY => ben_bytes!(self.trans_id),
            message::MESSAGE_TYPE_KEY => ben_bytes!(message::RESPONSE_TYPE_KEY),
            message::RESPONSE_TYPE_KEY => ben_map!{
                message::NODE_ID_KEY => ben_bytes!(self.node_id.as_ref()),
                message::NODES_KEY => ben_bytes!(self.nodes.nodes())
            }
        }).encode()
    }
}