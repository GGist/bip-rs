// TODO: Remove this when announces are implemented
#![allow(unused)]

use bip_bencode::{Bencode, BencodeConvert, Dictionary};
use bip_util::bt::{NodeId, InfoHash};

use message;
use message::request::{self, RequestValidate};
use error::DhtResult;

const PORT_KEY: &'static str = "port";
const IMPLIED_PORT_KEY: &'static str = "implied_port";

// TODO: Integrate the Token type into the request message.

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ConnectPort {
    Implied,
    Explicit(u16),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct AnnouncePeerRequest<'a> {
    trans_id: &'a [u8],
    node_id: NodeId,
    info_hash: InfoHash,
    token: &'a [u8],
    port: ConnectPort,
}

impl<'a> AnnouncePeerRequest<'a> {
    pub fn new(trans_id: &'a [u8],
               node_id: NodeId,
               info_hash: InfoHash,
               token: &'a [u8],
               port: ConnectPort)
               -> AnnouncePeerRequest<'a> {
        AnnouncePeerRequest {
            trans_id: trans_id,
            node_id: node_id,
            info_hash: info_hash,
            token: token,
            port: port,
        }
    }

    pub fn from_parts(rqst_root: &Dictionary<'a, Bencode<'a>>,
                      trans_id: &'a [u8])
                      -> DhtResult<AnnouncePeerRequest<'a>> {
        let validate = RequestValidate::new(trans_id);

        let node_id_bytes =
            try!(validate.lookup_and_convert_bytes(rqst_root, message::NODE_ID_KEY));
        let node_id = try!(validate.validate_node_id(node_id_bytes));

        let info_hash_bytes =
            try!(validate.lookup_and_convert_bytes(rqst_root, message::INFO_HASH_KEY));
        let info_hash = try!(validate.validate_info_hash(info_hash_bytes));

        let token = try!(validate.lookup_and_convert_bytes(rqst_root, message::TOKEN_KEY));
        let port = validate.lookup_and_convert_int(rqst_root, PORT_KEY);

        // Technically, the specification says that the value is either 0 or 1 but goes on to say that
        // if it is not zero, then the source port should be used. We will allow values other than 0 or 1.
        let response_port = match rqst_root.lookup(IMPLIED_PORT_KEY.as_bytes()).map(|n| n.int()) {
            Some(Some(n)) if n != 0 => ConnectPort::Implied,
            _ => {
                // If we hit this, the port either was not provided or it was of the wrong bencode type
                let port_number = try!(port) as u16;
                ConnectPort::Explicit(port_number)
            }
        };

        Ok(AnnouncePeerRequest::new(trans_id, node_id, info_hash, token, response_port))
    }

    pub fn transaction_id(&self) -> &'a [u8] {
        self.trans_id
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn info_hash(&self) -> InfoHash {
        self.info_hash
    }

    pub fn token(&self) -> &'a [u8] {
        self.token
    }

    pub fn connect_port(&self) -> ConnectPort {
        self.port
    }

    pub fn encode(&self) -> Vec<u8> {
        // In case a client errors out when the port key is not present, even when
        // implied port is specified, we will provide a dummy value in that case.
        let (displayed_port, implied_value) = match self.port {
            ConnectPort::Implied => (0, 1),
            ConnectPort::Explicit(n) => (n, 0),
        };

        (ben_map!{
            //message::CLIENT_TYPE_KEY => ben_bytes!(dht::CLIENT_IDENTIFICATION),
            message::TRANSACTION_ID_KEY => ben_bytes!(self.trans_id),
            message::MESSAGE_TYPE_KEY => ben_bytes!(message::REQUEST_TYPE_KEY),
            message::REQUEST_TYPE_KEY => ben_bytes!(request::ANNOUNCE_PEER_TYPE_KEY),
            request::REQUEST_ARGS_KEY => ben_map!{
                message::NODE_ID_KEY => ben_bytes!(self.node_id.as_ref()),
                IMPLIED_PORT_KEY => ben_int!(implied_value),
                message::INFO_HASH_KEY => ben_bytes!(self.info_hash.as_ref()),
                PORT_KEY => ben_int!(displayed_port as i64),
                message::TOKEN_KEY => ben_bytes!(self.token)
            }
        })
            .encode()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct AnnouncePeerResponse<'a> {
    trans_id: &'a [u8],
    node_id: NodeId,
}

impl<'a> AnnouncePeerResponse<'a> {
    pub fn new(trans_id: &'a [u8], node_id: NodeId) -> AnnouncePeerResponse<'a> {
        AnnouncePeerResponse {
            trans_id: trans_id,
            node_id: node_id,
        }
    }

    pub fn from_parts(rqst_root: &Dictionary<'a, Bencode<'a>>,
                      trans_id: &'a [u8])
                      -> DhtResult<AnnouncePeerResponse<'a>> {
        let validate = RequestValidate::new(&trans_id);

        let node_id_bytes =
            try!(validate.lookup_and_convert_bytes(rqst_root, message::NODE_ID_KEY));
        let node_id = try!(validate.validate_node_id(node_id_bytes));

        Ok(AnnouncePeerResponse::new(trans_id, node_id))
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
