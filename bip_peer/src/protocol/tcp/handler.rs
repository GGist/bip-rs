use mio::{EventLoop, EventLoopConfig};
       
pub fn spawn_protocol_handler(disk: &InactiveDiskManger, selector: PieceSelector) -> io::Result<Sender<IProtocolMessage<BTPeer>>> {
    
}