
use bip_util::send::TrySender;

use piece::{ISelectorMessage, OSelectorMessage, SelectorSender};
use protocol::OProtocolMessage;
use registration::LayerRegistration;
use token::Token;

pub struct PieceSelector;

impl<T> LayerRegistration<OSelectorMessage, T> for PieceSelector
    where T: Into<ISelectorMessage> + Send
{
    type SS2 = SelectorSender;

    fn register(&self, send: Box<TrySender<OSelectorMessage>>) -> SelectorSender {
        unimplemented!();
    }
}
