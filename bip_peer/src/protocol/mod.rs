//! Wire protocol implementation for the protocol layer.
#![allow(unused)]

use std::net::SocketAddr;
use std::sync::mpsc::SyncSender;
use std::io;

use bip_handshake::BTHandshaker;
use bip_util::bt::{PeerId, InfoHash};
use bip_util::send::{TrySender, SplitSender};
use rotor::Notifier;
use rotor::mio::tcp::TcpListener;

use disk::{DiskManager, IDiskMessage, ODiskMessage, DiskManagerAccess};
use selector::{OSelectorMessage, OSelectorMessageKind};
use message::standard::{HaveMessage, BitFieldMessage, RequestMessage, PieceMessage, CancelMessage};
use registration::LayerRegistration;
use token::Token;

mod context;
mod error;
mod wire;

pub use protocol::context::WireContext;
pub use protocol::wire::WireProtocol;

/// Spawn a TCP peer protocol handshaker.
pub fn spawn_tcp_handshaker<S, M, DLR, DL, SL>(metadata: S,
                                               listen: SocketAddr,
                                               pid: PeerId,
                                               disk: DL,
                                               select: SL)
                                               -> io::Result<BTHandshaker<S, M>>
    where S: TrySender<M> + 'static,
          M: Send,
          DLR: DiskManagerAccess + TrySender<IDiskMessage> + 'static,
          DL: LayerRegistration<ODiskMessage, IDiskMessage, SS2 = DLR> + 'static + Send,
          SL: LayerRegistration<OSelectorMessage, OProtocolMessage> + 'static + Send
{
    let wire_context = WireContext::new(disk, select);

    BTHandshaker::<S, M>::new::<WireProtocol<TcpListener, DLR>>(metadata, listen, pid, wire_context)
}

// ----------------------------------------------------------------------------//

/// Uniquely identifies a peer in the protocol layer.
///
/// Since peers could be connected to us over multiple connections
/// but may advertise the same peer id, we need to dis ambiguate
/// them by a combination of the address (ip + port) and peer id.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct PeerIdentifier {
    addr: SocketAddr,
    pid: PeerId,
}

impl PeerIdentifier {
    pub fn new(addr: SocketAddr, pid: PeerId) -> PeerIdentifier {
        PeerIdentifier {
            addr: addr,
            pid: pid,
        }
    }
}

// ----------------------------------------------------------------------------//

/// Messages that can be sent to the peer protocol layer.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum IProtocolMessage {
    /// Message from the disk manager to the protocol layer.
    DiskManager(ODiskMessage),
    /// Message from the piece manager to the protocol layer.
    PieceManager(OSelectorMessage),
}

impl From<IProtocolMessage> for ODiskMessage {
    fn from(data: IProtocolMessage) -> ODiskMessage {
        match data {
            IProtocolMessage::DiskManager(disk) => disk,
            IProtocolMessage::PieceManager(_) => unreachable!(),
        }
    }
}

impl From<ODiskMessage> for IProtocolMessage {
    fn from(data: ODiskMessage) -> IProtocolMessage {
        IProtocolMessage::DiskManager(data)
    }
}

impl From<IProtocolMessage> for OSelectorMessage {
    fn from(data: IProtocolMessage) -> OSelectorMessage {
        match data {
            IProtocolMessage::PieceManager(piece) => piece,
            IProtocolMessage::DiskManager(_) => unreachable!(),
        }
    }
}

impl From<OSelectorMessage> for IProtocolMessage {
    fn from(data: OSelectorMessage) -> IProtocolMessage {
        IProtocolMessage::PieceManager(data)
    }
}

// ----------------------------------------------------------------------------//

struct ProtocolSender {
    send: SyncSender<IProtocolMessage>,
    noti: Notifier,
}

impl ProtocolSender {
    fn new(send: SyncSender<IProtocolMessage>, noti: Notifier) -> ProtocolSender {
        ProtocolSender {
            send: send,
            noti: noti,
        }
    }
}

impl<T> TrySender<T> for ProtocolSender
    where T: Into<IProtocolMessage> + Send + From<IProtocolMessage>
{
    fn try_send(&self, data: T) -> Option<T> {
        let ret = TrySender::try_send(&self.send, data.into()).map(|data| data.into());

        if ret.is_none() {
            self.noti
                .wakeup()
                .expect("bip_peer: ProtocolSender Failed To Send Wakeup");
        }

        ret
    }
}

impl Clone for ProtocolSender {
    fn clone(&self) -> ProtocolSender {
        ProtocolSender {
            send: self.send.clone(),
            noti: self.noti.clone(),
        }
    }
}

// ----------------------------------------------------------------------------//

/// Combines a peer protocol layer message with a peer identifier.
pub struct OProtocolMessage {
    kind: OProtocolMessageKind,
    id: PeerIdentifier,
}

impl OProtocolMessage {
    pub fn new(id: PeerIdentifier, kind: OProtocolMessageKind) -> OProtocolMessage {
        OProtocolMessage {
            kind: kind,
            id: id,
        }
    }

    pub fn destroy(self) -> (PeerIdentifier, OProtocolMessageKind) {
        (self.id, self.kind)
    }
}

/// Enumeration of all messages originating from the peer protocol layer.
pub enum OProtocolMessageKind {
    /// Message that a peer has connected for the given InfoHash.
    PeerConnect(Box<TrySender<OSelectorMessage>>, InfoHash),
    /// Message that a peer has disconnected.
    PeerDisconnect,
    /// Message that a peer has choked us.
    PeerChoke,
    /// Message that a peer has unchoked us.
    PeerUnChoke,
    /// Message that a peer is interested in us.
    PeerInterested,
    /// Message that a peer is not interested in us.
    PeerUnInterested,
    /// Message that a peer has a specific piece.
    PeerHave(HaveMessage),
    /// Message that a peer has all pieces in the bitfield.
    PeerBitField(BitFieldMessage),
    /// Message that a peer has request a block from us.
    PeerRequest(RequestMessage),
    /// Message that a peer has sent a block to us.
    PeerPiece(Token, PieceMessage),
    /// Message that a peer has cancelled a block request from us.
    PeerCancel(CancelMessage),
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{self, Sender, Receiver};
    use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr, TcpStream};
    use std::io::{Write, Read};
    use std::thread;
    use std::time::Duration;
    use std::mem;

    use bip_handshake::{Handshaker, BTHandshaker};
    use bip_util::send::TrySender;
    use nom::IResult;
    use chan;

    use token::{TokenGenerator, Token};
    use disk::{ODiskMessage, IDiskMessage, DiskManager, DiskManagerAccess};
    use protocol::{OProtocolMessage, IProtocolMessage, OProtocolMessageKind, PeerIdentifier};
    use selector::{OSelectorMessage, ISelectorMessage, OSelectorMessageKind};
    use registration::LayerRegistration;
    use message::MessageType;
    use message::standard::{HaveMessage, RequestMessage};

    struct MockSender;
    impl<T: Send> TrySender<T> for MockSender {
        fn try_send(&self, data: T) -> Option<T> {
            Some(data)
        }
    }

    struct MockDiskManager {
        request_gen: TokenGenerator
    }
    impl MockDiskManager {
        fn new() -> MockDiskManager {
            MockDiskManager{ request_gen: TokenGenerator::new() }
        }
    }
    impl DiskManagerAccess for MockDiskManager {
        fn write_block(&self, token: Token, read_bytes: &[u8]) {
            unimplemented!()
        }

        fn read_block(&self, token: Token, write_bytes: &mut Write) {
            unimplemented!()
        }

        fn new_request_token(&mut self) -> Token {
            self.request_gen.generate()
        }
    }
    impl TrySender<IDiskMessage> for MockDiskManager {
        fn try_send(&self, msg: IDiskMessage) -> Option<IDiskMessage> {
            unimplemented!()
        }
    }

    struct MockDiskRegistration {
        namespace_gen: TokenGenerator
    }
    impl LayerRegistration<ODiskMessage, IDiskMessage> for MockDiskRegistration {
        type SS2 = MockDiskManager;

        fn register(&mut self, send: Box<TrySender<ODiskMessage>>) -> MockDiskManager {
            MockDiskManager::new()
        }
    }

    struct MockSelectionRegistration {
        send: Sender<OProtocolMessage>
    }
    impl LayerRegistration<OSelectorMessage, OProtocolMessage> for MockSelectionRegistration {
        type SS2 = Sender<OProtocolMessage>;

        fn register(&mut self, _send: Box<TrySender<OSelectorMessage>>) -> Sender<OProtocolMessage> {
            self.send.clone()
        }
    }

    fn mock_handshaker_setup() -> (BTHandshaker<Sender<()>, ()>, TcpStream, Receiver<OProtocolMessage>) {
        let (m_send, _m_recv): (Sender<()>, Receiver<()>) = mpsc::channel();

        let listen_ip = Ipv4Addr::new(127, 0, 0, 1);
        let listen_addr = SocketAddr::V4(SocketAddrV4::new(listen_ip, 0));
        let pid = [0u8; 20].into();

        let (protocol_send, protocol_recv) = mpsc::channel();
        let mock_select_registration = MockSelectionRegistration { send: protocol_send };
        let mock_disk_registration = MockDiskRegistration{ namespace_gen: TokenGenerator::new() };

        let handshaker = super::spawn_tcp_handshaker(m_send, listen_addr, pid, mock_disk_registration, mock_select_registration).unwrap();
        handshaker.register([0u8; 20].into());

        let mut stream = TcpStream::connect(SocketAddr::V4(SocketAddrV4::new(listen_ip, handshaker.port()))).unwrap();
        mock_initiate_handshake(&mut stream);

        thread::sleep(Duration::from_millis(100));
        
        (handshaker, stream, protocol_recv)
    }

    fn mock_initiate_handshake(stream: &mut TcpStream) {
        stream.write_all(&[19]);
        stream.write_all(&b"BitTorrent protocol"[..]);
        stream.write_all(&[0u8; 8 + 20 + 20][..]);

        stream.read(&mut [0u8; 1 + 19 + 8 + 20 + 20]);
    }

    fn assert_peer_connect(recv: &Receiver<OProtocolMessage>, stream: &TcpStream) -> (PeerIdentifier, Box<TrySender<OSelectorMessage>>) {
        let (peer_ident, msg_kind) = recv.try_recv().unwrap().destroy();

        let expected_peer_ident = PeerIdentifier::new(stream.local_addr().unwrap(), [0u8; 20].into());
        assert_eq!(peer_ident, expected_peer_ident);

        match msg_kind {
            OProtocolMessageKind::PeerConnect(peer_send, hash) => {
                assert_eq!(hash.as_ref(), &[0u8; 20]);

                (peer_ident, peer_send)
            }
            _ => panic!("Failed To Receive OProtocolMessageKind::PeerConnect"),
        }
    }

    #[test]
    fn positive_connect() {
        let (handshaker, stream, protocol_recv) = mock_handshaker_setup();
        let (peer_ident, peer_send) = assert_peer_connect(&protocol_recv, &stream);

        // We have to disconnect at the end of a test, otherwise, the state machine will see the stream was disconnected
        // and it will try to tell our now disconnected protocol_recv that a peer disconect happened which will trigger
        // a panic since it wont be able to send that message.
        assert!(peer_send.try_send(OSelectorMessage::new(peer_ident, OSelectorMessageKind::PeerDisconnect)).is_none());
    }

    #[test]
    fn positive_send_keep_alive() {
        let (handshaker, mut stream, protocol_recv) = mock_handshaker_setup();
        let (peer_ident, peer_send) = assert_peer_connect(&protocol_recv, &stream);

        // Cant have a similar test for receive keep alive because we don't propogate that message
        assert!(peer_send.try_send(OSelectorMessage::new(peer_ident, OSelectorMessageKind::PeerKeepAlive)).is_none());
        thread::sleep(Duration::from_millis(100));

        let mut recv_buffer = vec![0];
        stream.read_exact(&mut recv_buffer[..]).unwrap();

        assert_eq!(recv_buffer, vec![0]);
        assert!(peer_send.try_send(OSelectorMessage::new(peer_ident, OSelectorMessageKind::PeerDisconnect)).is_none());
    }

    #[test]
    fn positive_send_mutliple_messages() {
        let (handshaker, mut stream, protocol_recv) = mock_handshaker_setup();
        let (peer_ident, peer_send) = assert_peer_connect(&protocol_recv, &stream);

        let have_message = HaveMessage::new(100);
        let request_message = RequestMessage::new(10, 50, 100);

        assert!(peer_send.try_send(OSelectorMessage::new(peer_ident, OSelectorMessageKind::PeerHave(have_message))).is_none());
        assert!(peer_send.try_send(OSelectorMessage::new(peer_ident, OSelectorMessageKind::PeerRequest(request_message))).is_none());
        thread::sleep(Duration::from_millis(100));

        let mut recv_buffer = vec![0u8; 4 + 5 + 4 + 13];
        stream.read_exact(&mut recv_buffer[..]).unwrap();

        let (buffer, recv_first_message) = match MessageType::from_bytes(&recv_buffer) {
            IResult::Done(buffer, msg) => (buffer, msg),
            _ => panic!("Failed To Parse First Message"),
        };

        let recv_second_message = match MessageType::from_bytes(buffer) {
            IResult::Done(_, msg) => msg,
            _ => panic!("Failed To Parse Second Message"),
        };

        assert_eq!(recv_first_message, MessageType::Have(have_message));
        assert_eq!(recv_second_message, MessageType::Request(request_message));
        assert!(peer_send.try_send(OSelectorMessage::new(peer_ident, OSelectorMessageKind::PeerDisconnect)).is_none());
    }

    #[test]
    fn positive_recv_multiple_messages() {
        let (handshaker, mut stream, protocol_recv) = mock_handshaker_setup();
        let (peer_ident, peer_send) = assert_peer_connect(&protocol_recv, &stream);

        let have_message = HaveMessage::new(100);
        let request_message = RequestMessage::new(10, 50, 100);

        MessageType::Have(have_message).write_bytes(&mut stream).unwrap();
        MessageType::Request(request_message).write_bytes(&mut stream).unwrap();
        thread::sleep(Duration::from_millis(100));

        let (first_peer_ident, recv_first_message) = protocol_recv.try_recv().unwrap().destroy();
        let (second_peer_ident, recv_second_message) = protocol_recv.try_recv().unwrap().destroy();

        assert_eq!(first_peer_ident, peer_ident);
        match recv_first_message {
            OProtocolMessageKind::PeerHave(recv_have_message) => assert_eq!(recv_have_message, have_message),
            _ => panic!("Failed To Receive Have Message"),
        }

        assert_eq!(second_peer_ident, peer_ident);
        match recv_second_message {
            OProtocolMessageKind::PeerRequest(recv_request_message) => assert_eq!(recv_request_message, request_message),
            _ => panic!("Failed To Receive Request Message"),
        }

        assert!(peer_send.try_send(OSelectorMessage::new(peer_ident, OSelectorMessageKind::PeerDisconnect)).is_none());
    }
}
