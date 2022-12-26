use crate::manager::peer_info::PeerInfo;

error_chain! {
    types {
        PeerManagerError, PeerManagerErrorKind, PeerManagerResultExt, PeerManagerResult;
    }

    errors {
        PeerNotFound {
            info: PeerInfo
         } {
            description("Peer Was Not Found")
            display("Peer Was Not Found With PeerInfo {:?}", info)
        }
        
    }
}