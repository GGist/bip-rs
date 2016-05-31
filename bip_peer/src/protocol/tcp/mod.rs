use std::io::{self};

struct TCPProtocol {
    send: Sender<IProtocolMessage<BTPeer>>
}

impl TCPProtocol {
    pub fn new(disk: &InactiveDiskManager, selector: PieceSelector) -> io::Result<TCPProtocol> {
        let send = try!(handler::spawn_protocol_handler(disk, selector));
        
        TCPProtocol{ send: send }
    }
    
    pub fn peer_sender(&self) -> PeerSender {
        PeerSender{ send: self.send.clone() }
    }
}