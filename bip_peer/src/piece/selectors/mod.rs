
use bip_util::sender::Sender;

use piece::{ISelectorMessage, OSelectorMessage, SelectorSender};
use protocol::OProtocolMessage;
use registration::LayerRegistration;
use token::Token;

pub struct PieceSelector;

impl<T> LayerRegistration<OSelectorMessage, T> for PieceSelector
    where T: Into<ISelectorMessage> + Send
{
    type SS2 = SelectorSender;

    fn register(&self, send: Box<Sender<OSelectorMessage>>) -> SelectorSender {
        unimplemented!();
    }
}
