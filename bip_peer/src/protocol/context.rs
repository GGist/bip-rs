use std::sync::mpsc::{Receiver, TryRecvError};

use bip_util::bt::{PeerId, InfoHash};
use bip_util::send::TrySender;
use rotor::{Machine, Void, Scope, Response, EventSet};
use rotor_stream::{Accepted, StreamSocket};

use disk::{InactiveDiskManager, ODiskMessage, ActiveDiskManager, IDiskMessage};
use protocol::OProtocolMessage;
use selector::OSelectorMessage;
use registration::LayerRegistration;

pub struct WireContext {
    disk: Box<LayerRegistration<ODiskMessage, IDiskMessage, SS2 = ActiveDiskManager> + Send>,
    sele: Box<TrySender<OProtocolMessage> + Send>,
}

impl WireContext {
    pub fn new<D, S>(disk: D, selector: S) -> WireContext
        where D: LayerRegistration<ODiskMessage, IDiskMessage, SS2 = ActiveDiskManager> + 'static + Send,
              S: LayerRegistration<OSelectorMessage, OProtocolMessage> + 'static + Send
    {
        // Selector will not send anything through this channel, instead, it will wait to
        // receive a PeerConnect message with a sender for that peer. Peers will send back
        // to the selector through this selector channel (to reduce the number of channels
        // created) and will be dis ambiguated with the PeerIdentifier (corresponds to a unique peer).
        let sel_send = Box::new(selector.register(Box::new(UnusedSender)));

        WireContext {
            disk: Box::new(disk),
            sele: sel_send,
        }
    }

    pub fn register_disk(&self, send: Box<TrySender<ODiskMessage>>) -> ActiveDiskManager {
        self.disk.register(send)
    }

    pub fn send_selector(&self, msg: OProtocolMessage) {
        assert!(self.sele.try_send(msg).is_none());
    }
}

// ----------------------------------------------------------------------------//

struct UnusedSender;

impl TrySender<OSelectorMessage> for UnusedSender {
    fn try_send(&self, msg: OSelectorMessage) -> Option<OSelectorMessage> {
        panic!("bip_peer: Selector Tried To Send Message Through UnusedSender")
    }
}
