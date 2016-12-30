use std::io::{self};

use bip_bencode::{BencodeConvertError};

use message::error::{ErrorMessage};

error_chain! {
    types {
        DhtError, DhtErrorKind, DhtResultExt, DhtResult;
    }

    foreign_links {
        Bencode(BencodeConvertError);
        Io(io::Error);
    }

    errors {
        InvalidMessage{
            code: String
        } {
            description("Node Sent An Invalid Message")
            display("Node Sent An Invalid Message With Message Code {}", code)
        }
        InvalidResponse{
            details: String
        } {
            description("Node Sent Us An Invalid Response")
            display("Node Sent Us An Invalid Response: {}", details)
        }
        UnsolicitedResponse {
            description("Node Sent Us An Unsolicited Response")
            display("Node Sent Us An Unsolicited Response")
        }
        InvalidRequest {
            msg: ErrorMessage<'static>
        } {
            description("Node Sent Us An Invalid Request Message")
            display("Node Sent Us An Invalid Request Message With Code {:?} And Message {}", msg.error_code(), msg.error_message())
        }
    }
}