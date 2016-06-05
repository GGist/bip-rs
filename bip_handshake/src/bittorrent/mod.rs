use std::collections::HashSet;
use std::io;
use std::sync::{Arc, RwLock};
use std::net::SocketAddr;

use bip_util::bt::{PeerId, InfoHash};
use bip_util::sender::{Sender, PrioritySender};
use mio;
use mio::tcp::TcpStream;

use bittorrent::handler::Task;
use handshaker::Handshaker;

mod handler;

const MAX_PROTOCOL_LEN: usize = 255;
const BTP_10_PROTOCOL: &'static str = "BitTorrent protocol";

/// Bittorrent TCP peer that has been handshaken.
#[derive(Debug)]
pub struct BTPeer {
    stream: TcpStream,
    pid: PeerId,
    hash: InfoHash,
}

impl BTPeer {
    /// Create a new BTPeer container.
    pub fn new(stream: TcpStream, hash: InfoHash, pid: PeerId) -> BTPeer {
        BTPeer {
            stream: stream,
            hash: hash,
            pid: pid,
        }
    }

    /// Destroy the BTPeer container and return the contained objects.
    pub fn destroy(self) -> (TcpStream, InfoHash, PeerId) {
        (self.stream, self.hash, self.pid)
    }
}

// ----------------------------------------------------------------------------//

pub struct MioSender<T: Send> {
    send: mio::Sender<T>
}

impl<T: Send> MioSender<T> {
    pub fn new(send: mio::Sender<T>) -> MioSender<T> {
        MioSender{ send: send }
    }
}

impl<T: Send> Sender<T> for MioSender<T> {
    #[allow(unused)]
    fn send(&self, data: T) {
        self.send.send(data);
    }
}

// ----------------------------------------------------------------------------//

/// Bittorrent TCP peer handshaker.
pub struct BTHandshaker<T>
    where T: Send
{
    inner: Arc<InnerBTHandshaker<T>>,
}

impl<T> Clone for BTHandshaker<T>
    where T: Send
{
    fn clone(&self) -> BTHandshaker<T> {
        BTHandshaker { inner: self.inner.clone() }
    }
}

pub struct InnerBTHandshaker<T>
    where T: Send
{
    // Using a priority channel because shutdown messages cannot
    // afford to be lost. Generally the handler will set the capacity
    // on the channel to 1 less than the real capacity which gives us
    // room for a shutdown message in the absolute worst case.
    send: PrioritySender<MioSender<Task<T>>, Task<T>>,
    interest: Arc<RwLock<HashSet<InfoHash>>>,
    port: u16,
    pid: PeerId,
}

impl<T> Drop for InnerBTHandshaker<T>
    where T: Send
{
    fn drop(&mut self) {
        self.send.prioritized_send(Task::Shutdown);
    }
}

impl<T> BTHandshaker<T>
    where T: From<BTPeer> + Send + 'static
{
    /// Create a new BTHandshaker with the given PeerId and bind address which will
    /// forward metadata and handshaken connections onto the provided sender.
    pub fn new<S>(send: S, listen: SocketAddr, pid: PeerId) -> io::Result<BTHandshaker<T>>
        where S: Sender<T> + 'static
    {
        BTHandshaker::with_protocol(send, listen, pid, BTP_10_PROTOCOL)
    }

    /// Similar to BTHandshaker::new() but allows a client to specify a custom protocol
    /// that the handshaker will specify during the handshake.
    ///
    /// Panics if the length of the provided protocol exceeds 255 bytes.
    pub fn with_protocol<S>(send: S, listen: SocketAddr, pid: PeerId, protocol: &'static str) -> io::Result<BTHandshaker<T>>
        where S: Sender<T> + 'static
    {
        if protocol.len() > MAX_PROTOCOL_LEN {
            panic!("bip_handshake: BTHandshaker Protocol Length Cannot Exceed {}", MAX_PROTOCOL_LEN);
        }
        let interest = Arc::new(RwLock::new(HashSet::new()));
        let (send, port) = try!(handler::spawn_handshaker(send, listen, pid, protocol, interest.clone()));

        Ok(BTHandshaker {
            inner: Arc::new(InnerBTHandshaker {
                send: send,
                interest: interest,
                port: port,
                pid: pid,
            }),
        })
    }

    /// Register interest for the given InfoHash allowing connections for the given InfoHash to succeed. Connections
    /// already in the handshaking process may not be effected by this call.
    ///
    /// By default, a BTHandshaker will be interested in zero InfoHashs.
    ///
    /// This is a blocking operation.
    pub fn register_hash(&self, hash: InfoHash) {
        self.inner.interest.write().unwrap().insert(hash);
    }

    /// Deregister interest for the given InfoHash causing connections for the given InfoHash to fail. Connections
    /// already in the handshaking process may not be effected by this call.
    ///
    /// By default, a BTHandshaker will be interested in zero InfoHashs.
    ///
    /// This is a blocking operation.
    pub fn deregister_hash(&self, hash: InfoHash) {
        self.inner.interest.write().unwrap().remove(&hash);
    }
}

impl<T> Handshaker for BTHandshaker<T>
    where T: Send
{
    type MetadataEnvelope = T;

    fn id(&self) -> PeerId {
        self.inner.pid
    }

    fn port(&self) -> u16 {
        self.inner.port
    }

    fn connect(&mut self, expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr) {
        self.inner.send.send(Task::Connect(expected, hash, addr));
    }

    fn metadata(&mut self, data: Self::MetadataEnvelope) {
        self.inner.send.send(Task::Metadata(data));
    }
}

#[cfg(test)]
mod tests {
    use std::mem;
    use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
    use std::sync::mpsc::{self, TryRecvError, Sender, Receiver};
    use std::thread;
    use std::time::Duration;

    use bip_util::bt;

    use bittorrent::{BTHandshaker, BTPeer};
    use handshaker::Handshaker;

    #[test]
    fn positive_make_conenction() {
        // Assign a listen address and peer id for each handshaker
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let (addr_one, addr_two) = (SocketAddr::V4(SocketAddrV4::new(ip, 0)), SocketAddr::V4(SocketAddrV4::new(ip, 0)));
        let (peer_id_one, peer_id_two) = ([1u8; bt::PEER_ID_LEN].into(), [0u8; bt::PEER_ID_LEN].into());

        // Create receiving channels
        let (send_one, recv_one): (Sender<BTPeer>, Receiver<BTPeer>) = mpsc::channel();
        let (send_two, recv_two): (Sender<BTPeer>, Receiver<BTPeer>) = mpsc::channel();

        // Create two handshakers
        let mut handshaker_one = BTHandshaker::new(send_one, addr_one, peer_id_one).unwrap();
        let handshaker_two = BTHandshaker::new(send_two, addr_two, peer_id_two).unwrap();

        // Register both handshakers for the same info hash
        let info_hash = [0u8; bt::INFO_HASH_LEN].into();
        handshaker_one.register_hash(info_hash);
        handshaker_two.register_hash(info_hash);

        // Allow the handshakers to connect to each other
        thread::sleep(Duration::from_millis(100));

        // Get the address and bind port for handshaker two
        let handshaker_two_addr = SocketAddr::V4(SocketAddrV4::new(ip, handshaker_two.port()));

        // Connect to handshaker two from handshaker one
        handshaker_one.connect(Some(peer_id_two), info_hash, handshaker_two_addr);

        // Allow the handshakers to connect to each other
        thread::sleep(Duration::from_millis(100));

        // Should receive the peer from both handshakers
        match (recv_one.try_recv(), recv_two.try_recv()) {
            (Ok(_), Ok(_)) => (),
            _ => panic!("Failed to find peers on one or both handshakers..."),
        };
    }

    #[test]
    fn positive_shutdown_on_drop() {
        // Create our handshaker addresses and ids
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let peer_id = [0u8; bt::PEER_ID_LEN].into();

        let (send, recv): (Sender<BTPeer>, Receiver<BTPeer>) = mpsc::channel();

        // Create the handshaker
        let handshaker = BTHandshaker::new(send, addr, peer_id).unwrap();

        // Subscribe to a specific info hash
        let info_hash = [1u8; bt::INFO_HASH_LEN].into();
        handshaker.register_hash(info_hash);

        // Clone the handshaker so there are two copies of it
        let handshaker_clone = handshaker.clone();

        // Drop one of the copies of the handshaker
        mem::drop(handshaker_clone);

        // Allow the event loop to process the shutdown that should NOT have been fired
        thread::sleep(Duration::from_millis(100));

        // Make sure that we can still receive values on our channel
        assert_eq!(recv.try_recv().unwrap_err(), TryRecvError::Empty);

        // Drop the other copy of the handshaker so there are no more copies around
        mem::drop(handshaker);

        // Allow the event loop to process the shutdown that should have been fired
        thread::sleep(Duration::from_millis(100));

        // Assert that the channel hunp up (because of the shutdown)
        assert_eq!(recv.try_recv().unwrap_err(), TryRecvError::Disconnected);
    }
}
