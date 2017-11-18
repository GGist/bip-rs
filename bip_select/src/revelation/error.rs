//! Module for revelation error types.

use bip_handshake::InfoHash;
use bip_peer::PeerInfo;

error_chain! {
    types {
        RevealError, RevealErrorKind, RevealResultExt;
    }

    errors {
        InvalidMessage {
            info:    PeerInfo,
            message: String
        } {
            description("Peer Sent An Invalid Message")
            display("Peer {:?} Sent An Invalid Message: {:?}", info, message)
        }
        InvalidMetainfoExists {
            hash: InfoHash
        } {
            description("Metainfo Has Already Been Added")
            display("Metainfo With Hash {:?} Has Already Been Added", hash)
        }
        InvalidMetainfoNotExists {
            hash: InfoHash
        } {
            description("Metainfo Was Not Already Added")
            display("Metainfo With Hash {:?} Was Not Already Added", hash)
        }
        InvalidPieceOutOfRange {
            hash: InfoHash,
            index: u64
        } {
            description("Piece Index Was Out Of Range")
            display("Piece Index {:?} Was Out Of Range For Hash {:?}", index, hash)
        }
    }
}
