use bip_peer::PeerInfo;

error_chain! {
    types {
        DiscoveryError, DiscoveryErrorKind, DiscoveryResultExt;
    }

    errors {
        InvalidMessage {
            info:    PeerInfo,
            message: String
        } {
            description("Peer Sent An Invalid Message")
            display("Peer {:?} Sent An Invalid Message: {:?}", info, message)
        }
    }
}
