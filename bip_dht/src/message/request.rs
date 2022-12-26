use bip_bencode::{Bencode, BencodeConvert, BencodeConvertError, Dictionary};
use bip_util::bt::{InfoHash, NodeId};

use crate::error::{DhtError, DhtErrorKind, DhtResult};
use crate::message;
use crate::message::announce_peer::AnnouncePeerRequest;
use crate::message::error::{ErrorCode, ErrorMessage};
use crate::message::find_node::FindNodeRequest;
use crate::message::get_peers::GetPeersRequest;
use crate::message::ping::PingRequest;

pub const REQUEST_ARGS_KEY: &str = "a";

// Top level request methods
pub const PING_TYPE_KEY: &str = "ping";
pub const FIND_NODE_TYPE_KEY: &str = "find_node";
pub const GET_PEERS_TYPE_KEY: &str = "get_peers";
pub const ANNOUNCE_PEER_TYPE_KEY: &str = "announce_peer";
// const GET_DATA_TYPE_KEY:          &'static str = "get";
// const PUT_DATA_TYPE_KEY:          &'static str = "put";

// ----------------------------------------------------------------------------//

pub struct RequestValidate<'a> {
    trans_id: &'a [u8],
}

impl<'a> RequestValidate<'a> {
    pub fn new(trans_id: &'a [u8]) -> RequestValidate<'a> {
        RequestValidate { trans_id }
    }

    pub fn validate_node_id(&self, node_id: &[u8]) -> DhtResult<NodeId> {
        NodeId::from_hash(node_id).map_err(|_| {
            let error_msg = ErrorMessage::new(
                self.trans_id.to_owned(),
                ErrorCode::ProtocolError,
                format!("Node ID With Length {} Is Not Valid", node_id.len()),
            );

            DhtError::from_kind(DhtErrorKind::InvalidRequest { msg: error_msg })
        })
    }

    pub fn validate_info_hash(&self, info_hash: &[u8]) -> DhtResult<InfoHash> {
        InfoHash::from_hash(info_hash).map_err(|_| {
            let error_msg = ErrorMessage::new(
                self.trans_id.to_owned(),
                ErrorCode::ProtocolError,
                format!("InfoHash With Length {} Is Not Valid", info_hash.len()),
            );

            DhtError::from_kind(DhtErrorKind::InvalidRequest { msg: error_msg })
        })
    }
}

impl<'a> BencodeConvert for RequestValidate<'a> {
    type Error = DhtError;

    fn handle_error(&self, error: BencodeConvertError) -> DhtError {
        error.into()
    }
}

// ----------------------------------------------------------------------------//

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum RequestType<'a> {
    Ping(PingRequest<'a>),
    FindNode(FindNodeRequest<'a>),
    GetPeers(GetPeersRequest<'a>),
    AnnouncePeer(AnnouncePeerRequest<'a>), /* GetData(GetDataRequest<'a>),
                                            * PutData(PutDataRequest<'a>) */
}

impl<'a> RequestType<'a> {
    pub fn from_parts(root: &dyn Dictionary<'a, Bencode<'a>>, trans_id: &'a [u8], rqst_type: &str) -> DhtResult<RequestType<'a>> {
        let validate = RequestValidate::new(trans_id);
        let rqst_root = validate.lookup_and_convert_dict(root, REQUEST_ARGS_KEY)?;

        match rqst_type {
            PING_TYPE_KEY => {
                let ping_rqst = PingRequest::from_parts(rqst_root, trans_id)?;
                Ok(RequestType::Ping(ping_rqst))
            },
            FIND_NODE_TYPE_KEY => {
                let find_node_rqst = FindNodeRequest::from_parts(rqst_root, trans_id, message::TARGET_ID_KEY)?;
                Ok(RequestType::FindNode(find_node_rqst))
            },
            GET_PEERS_TYPE_KEY => {
                let get_peers_rqst = GetPeersRequest::from_parts(rqst_root, trans_id)?;
                Ok(RequestType::GetPeers(get_peers_rqst))
            },
            ANNOUNCE_PEER_TYPE_KEY => {
                let announce_peer_rqst = AnnouncePeerRequest::from_parts(rqst_root, trans_id)?;
                Ok(RequestType::AnnouncePeer(announce_peer_rqst))
            },
            // GET_DATA_TYPE_KEY => {
            // let get_data_rqst = try!(GetDataRequest::new(rqst_root, trans_id));
            // Ok(RequestType::GetData(get_data_rqst))
            // },
            // PUT_DATA_TYPE_KEY => {
            // let put_data_rqst = try!(PutDataRequest::new(rqst_root, trans_id));
            // Ok(RequestType::PutData(put_data_rqst))
            // },
            unknown => {
                if let Some(target_key) = forward_compatible_find_node(rqst_root) {
                    let find_node_rqst = FindNodeRequest::from_parts(rqst_root, trans_id, target_key)?;
                    Ok(RequestType::FindNode(find_node_rqst))
                } else {
                    let error_message = ErrorMessage::new(
                        trans_id.to_owned(),
                        ErrorCode::MethodUnknown,
                        format!("Received Unknown Request Method: {}", unknown),
                    );

                    Err(DhtError::from_kind(DhtErrorKind::InvalidRequest { msg: error_message }))
                }
            },
        }
    }
}

/// Mainline dht extension for forward compatibility.
///
/// Treat unsupported messages with either a target id key or info hash key as find node messages.
fn forward_compatible_find_node<'a>(rqst_root: &dyn Dictionary<'a, Bencode<'a>>) -> Option<&'static str> {
    match (
        rqst_root.lookup(message::TARGET_ID_KEY.as_bytes()),
        rqst_root.lookup(message::INFO_HASH_KEY.as_bytes()),
    ) {
        (Some(_), _) => Some(message::TARGET_ID_KEY),
        (_, Some(_)) => Some(message::INFO_HASH_KEY),
        (None, None) => None,
    }
}
