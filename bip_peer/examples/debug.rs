extern crate bip_handshake;
extern crate bip_peer;
extern crate bip_util;
extern crate chan;

use std::io::Write;
use std::sync::mpsc::{self, Sender, Receiver};

use bip_handshake::{Handshaker};
use bip_peer::disk::{ODiskMessage, IDiskMessage, DiskManagerAccess};
use bip_peer::protocol::{self, OProtocolMessage, OProtocolMessageKind};
use bip_peer::selector::{OSelectorMessage, OSelectorMessageKind};
use bip_peer::LayerRegistration;
use bip_peer::token::{Token};
use bip_util::send::TrySender;

struct MockDiskManager;
impl DiskManagerAccess for MockDiskManager {
    fn write_block(&self, _token: Token, _read_bytes: &[u8]) {
        unimplemented!()
    }

    fn read_block(&self, _token: Token, _write_bytes: &mut Write) {
        unimplemented!()
    }

    fn new_request_token(&mut self) -> Token {
        unimplemented!()
    }
}
impl TrySender<IDiskMessage> for MockDiskManager {
    fn try_send(&self, _msg: IDiskMessage) -> Option<IDiskMessage> {
        unimplemented!()
    }
}

struct MockDiskRegistration;
impl LayerRegistration<ODiskMessage, IDiskMessage> for MockDiskRegistration {
    type SS2 = MockDiskManager;

    fn register(&mut self, _send: Box<TrySender<ODiskMessage>>) -> MockDiskManager {
        MockDiskManager
    }
}

struct MockSelectionRegistration {
    send: Sender<OProtocolMessage>,
}
impl LayerRegistration<OSelectorMessage, OProtocolMessage> for MockSelectionRegistration {
    type SS2 = Sender<OProtocolMessage>;

    fn register(&mut self, _send: Box<TrySender<OSelectorMessage>>) -> Sender<OProtocolMessage> {
        self.send.clone()
    }
}

fn main() {
    let (send, _recv): (Sender<()>, Receiver<()>) = mpsc::channel();

    let (protocol_send, protocol_recv) = mpsc::channel();
    let selection_registration = MockSelectionRegistration{ send: protocol_send };
    let disk_registration = MockDiskRegistration;

    let pid = ['-' as u8, 'U' as u8, 'T' as u8, '5' as u8, '5' as u8, '5' as u8, '5' as u8, '-' as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].into();

    let mut handshaker = protocol::spawn_tcp_handshaker(send, "0.0.0.0:0".parse().unwrap(), pid, disk_registration, selection_registration).unwrap();

    let hash = [0xf8, 0x8e, 0xd0, 0xc1, 0x6c, 0xf7, 0xf4, 0x52, 0xa5, 0xc7, 0x37, 0xa0, 0xb7, 0x50, 0x3f, 0x92, 0x5e, 0x11, 0xfe, 0x00].into();
    handshaker.register(hash);

    handshaker.connect(None, hash, "10.0.0.18:55194".parse().unwrap());

    let (id, kind) = protocol_recv.recv().unwrap().destroy();
    let protocol_send = match kind {
        OProtocolMessageKind::PeerConnect(peer_send, _) => peer_send,
        _ => panic!("1")
    };

    //thread::sleep(Duration::from_millis(2000));

    protocol_send.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerKeepAlive));

    loop {
        match protocol_recv.recv().unwrap().destroy().1 {
            OProtocolMessageKind::PeerChoke => println!("PeerChoke"),
            OProtocolMessageKind::PeerUnChoke => println!("PeerUnChoke"),
            OProtocolMessageKind::PeerInterested => println!("PeerInterested"),
            OProtocolMessageKind::PeerUnInterested => println!("PeerUnInterested"),
            OProtocolMessageKind::PeerHave(..) => println!("PeerHave"),
            OProtocolMessageKind::PeerBitField(b) => println!("PeerBitField {:?}", b),
            OProtocolMessageKind::PeerRequest(..) => println!("PeerRequest"),
            OProtocolMessageKind::PeerPiece(..) => println!("PeerPiece"),
            OProtocolMessageKind::PeerCancel(..) => println!("PeerCancel"),
            OProtocolMessageKind::PeerDisconnect => panic!("PeerDisconnect"),
            _ => panic!("ASD")
        }
    }
}