

pub struct ProtocolSender {
    id:   Token,
    send: Sender<IPieceMessage>
}

impl Sender<OProtocolMessage> for ProtocolSender {
    fn send(&self, data: OProtocolMessage) {
        self.send.send(IPieceMessage::Protocol(self.id, data));
    }
}

//----------------------------------------------------------------------------//

pub struct PieceSelector;

impl PieceSelector {
    pub fn register_protocol(&self, send: PieceSender) -> ProtocolSender {
        unimplemented!();
    }
}