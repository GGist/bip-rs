use bip_bencode::{Bencode, BencodeConvert, Dictionary, BencodeConvertError};

use message::{self};
use message::compact_info::{CompactNodeInfo, CompactValueInfo};
use message::ping::{PingResponse};
use message::find_node::{FindNodeResponse};
use message::get_peers::{GetPeersResponse};
use message::announce_peer::{AnnouncePeerResponse};
use error::{DhtError, DhtErrorKind, DhtResult};

pub const RESPONSE_ARGS_KEY: &'static str = "r";

//----------------------------------------------------------------------------//

pub struct ResponseValidate<'a> {
    trans_id: &'a [u8]
}

impl<'a> ResponseValidate<'a> {
    pub fn new(trans_id: &'a [u8]) -> ResponseValidate<'a> {
        ResponseValidate{ trans_id: trans_id }
    }
    
    /// Validate the given node id.
    pub fn validate_node_id(&self, node_id: &[u8]) -> DhtResult<()> {
        if !message::is_valid_node_id(node_id) {
            Err(DhtError::with_detail(DhtErrorKind::InvalidResponse, "Found Node ID With Invalid Length",
                node_id.len().to_string()))
        } else {
            Ok(())
        }
    }
    
    /// Validate the given nodes string which should be IPv4 compact 
    pub fn validate_nodes<'b>(&self, nodes: &'b [u8]) -> DhtResult<CompactNodeInfo<'b>> {
        CompactNodeInfo::new(nodes).map_err( |_|
            DhtError::new(DhtErrorKind::InvalidResponse, "Found Nodes Structure With Wrong Multiple Of Bytes")
        )
    }
    
    pub fn validate_values<'b>(&self, values: &'b [Bencode<'a>]) -> DhtResult<CompactValueInfo<'b>> {
        for bencode in values.iter() {
            match bencode.bytes() {
                Some(_) => (),
                None    => return Err(DhtError::new(DhtErrorKind::InvalidResponse,
                    "Found Values Structure Element With Wrong Bencode Type"))
            }
        }
    
        CompactValueInfo::new(values).map_err( |_|
            DhtError::new(DhtErrorKind::InvalidResponse, "Found values Structure Element With Wrong Number Of Bytes")
        )
    }
}

impl<'a> BencodeConvert for ResponseValidate<'a> {
    type Error = DhtError;
    
    fn handle_error(&self, error: BencodeConvertError) -> DhtError {
        DhtError::with_detail(DhtErrorKind::InvalidResponse, error.desc(), error.key().to_owned())
    }
}

//----------------------------------------------------------------------------//

pub enum ExpectedResponse {
    Ping,
    FindNode,
    GetPeers,
    AnnouncePeer,
    GetData,
    PutData,
    None
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ResponseType<'a> {
    Ping(PingResponse<'a>),
    FindNode(FindNodeResponse<'a>),
    GetPeers(GetPeersResponse<'a>),
    AnnouncePeer(AnnouncePeerResponse<'a>)
    /*GetData(GetDataResponse<'a>),
    PutData(PutDataResponse<'a>)*/
}

impl<'a> ResponseType<'a> {
    pub fn from_parts(root: &'a Dictionary<'a, Bencode<'a>>, trans_id: &'a [u8], rsp_type: ExpectedResponse)
        -> DhtResult<ResponseType<'a>> {
        let validate = ResponseValidate::new(trans_id);
        let rqst_root = try!(validate.lookup_and_convert_dict(root, RESPONSE_ARGS_KEY));
        
        match rsp_type {
            ExpectedResponse::Ping => {
                let ping_rsp = try!(PingResponse::from_parts(rqst_root, trans_id));
                Ok(ResponseType::Ping(ping_rsp))
            },
            ExpectedResponse::FindNode => {
                let find_node_rsp = try!(FindNodeResponse::from_parts(rqst_root, trans_id));
                Ok(ResponseType::FindNode(find_node_rsp))
            },
            ExpectedResponse::GetPeers => {
                let get_peers_rsp = try!(GetPeersResponse::from_parts(rqst_root, trans_id));
                Ok(ResponseType::GetPeers(get_peers_rsp))
            },
            ExpectedResponse::AnnouncePeer => {
                let announce_peer_rsp = try!(AnnouncePeerResponse::from_parts(rqst_root, trans_id));
                Ok(ResponseType::AnnouncePeer(announce_peer_rsp))
            },
            ExpectedResponse::GetData => {
                unimplemented!();
            },
            ExpectedResponse::PutData => {
                unimplemented!();
            },
            ExpectedResponse::None => {
                Err(DhtError::new(DhtErrorKind::UnsolicitedResponse, "Failed To Match Node Response To A Request"))
            }
        }
    }
    
    pub fn transaction_id(&self) -> &'a [u8] {
        match self {
            &ResponseType::Ping(ref n)          => n.transaction_id(),
            &ResponseType::FindNode(ref n)      => n.transaction_id(),
            &ResponseType::GetPeers(ref n)      => n.transaction_id(),
            &ResponseType::AnnouncePeer(ref n)  => n.transaction_id()
            /*&ResponseType::GetData(ref n)       => n.transaction_id(),
            &ResponseType::PutData(ref n)       => n.transaction_id()*/
        }
    }
}