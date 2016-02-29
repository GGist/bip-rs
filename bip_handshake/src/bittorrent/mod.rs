use std::io::{self, Error, ErrorKind};
use std::net::{SocketAddr};
use std::sync::{Arc};
use std::sync::mpsc::{self};

use bip_util::bt::{InfoHash, PeerId};
use mio::{self};
use mio::tcp::{TcpListener, TcpStream};

use handshaker::{Handshaker};
use bittorrent::handler::{HandlerTask};

mod connection;
mod handler;

const MAX_PROTOCOL_LEN: usize        = 255;
const BTP_10_PROTOCOL:  &'static str = "BitTorrent protocol";

/// Handshaker that uses the bittorrent handshake protocol.
///
/// 
pub struct BTHandshaker<T> where T: Send {
    inner: Arc<InnerBTHandshaker<T>>
}

// TODO: For some reason, it would not let us derive this...
impl<T> Clone for BTHandshaker<T> where T: Send {
    fn clone(&self) -> BTHandshaker<T> {
        BTHandshaker{ inner: self.inner.clone() }
    }
}

/// Used so that Drop executes just once and shuts down the event loop.
pub struct InnerBTHandshaker<T> where T: Send {
    send:    mio::Sender<HandlerTask<T>>,
    port:    u16,
    peer_id: PeerId
}

impl<T> Drop for InnerBTHandshaker<T> where T: Send {
    fn drop(&mut self) {
        if self.send.send(HandlerTask::Shutdown).is_err() {
            error!("bip_handshake: Error shutting down event loop...");
        }
    }
}

impl<T> BTHandshaker<T> where T: From<TcpStream> + Send + 'static {
    /// Create a new BTHandshaker using the standard bittorrent protocol.
    pub fn new(listen_addr: &SocketAddr, peer_id: PeerId) -> io::Result<BTHandshaker<T>> {
        BTHandshaker::with_protocol(listen_addr, peer_id, BTP_10_PROTOCOL)
    }
    
    /// Create a new BTHandshaker using a custom protocol.
    ///
    /// Length of the custom protocol MUST be less than or equal to 255 (fit within a byte).
    pub fn with_protocol(listen_addr: &SocketAddr, peer_id: PeerId, protocol: &'static str) -> io::Result<BTHandshaker<T>> {
        if protocol.len() > MAX_PROTOCOL_LEN {
            Err(Error::new(ErrorKind::InvalidInput, "Protocol Length Exceeds Maximum Length"))
        } else {
            let listener = try!(TcpListener::bind(listen_addr));
            // Important to get the full address from the listener so we get the RESOLVED port in
            // case the user specified port 0, which would tell the os to bind to the next free port.
            let listen_port = try!(listener.local_addr()).port();
            
            let send = try!(handler::create_handshake_handler(listener, peer_id, protocol));
            let inner = InnerBTHandshaker{ send: send, port: listen_port, peer_id: peer_id };
            
            Ok(BTHandshaker{ inner: Arc::new(inner) })
        }
    }
}

impl<T> Handshaker for BTHandshaker<T> where T: Send {
    type Stream = mpsc::Receiver<(T, PeerId)>;
    
    fn id(&self) -> PeerId {
        self.inner.peer_id
    }
    
    fn port(&self) -> u16 {
        self.inner.port
    }
    
    fn connect(&mut self, expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr) {
        if self.inner.send.send(HandlerTask::ConnectPeer(expected, hash, addr)).is_err() {
            error!("bip_handshake: Error sending a connect peer message to event loop...");
        }
    }
    
    fn filter<F>(&mut self, process: Box<F>) where F: Fn(&SocketAddr) -> bool + Send + 'static {
        if self.inner.send.send(HandlerTask::RegisterFilter(process)).is_err() {
            error!("bip_handshake: Error sending a filter peer message to event loop...");
        }
    }
    
    fn stream(&self, hash: InfoHash) -> mpsc::Receiver<(T, PeerId)> {
        let (send, recv) = mpsc::channel();
        if self.inner.send.send(HandlerTask::RegisterSender(hash, send)).is_err() {
            error!("bip_handshake: Error sending a register sender message to event loop...");
        }
        
        recv
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{self};
    use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr, TcpListener};
    use std::sync::mpsc::{self, TryRecvError};
    use std::thread::{self};
    use std::time::{Duration};
    
    use bip_util::bt::{self};
    use mio::tcp::{TcpStream};
    
    use bittorrent::{BTHandshaker};
    use bittorrent::connection::{self};
    use bittorrent::handler::{self};
    use handshaker::{Handshaker};
    
    #[test]
    fn positive_make_conenction() {
        // Assign a listen address and peer id for each handshaker
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let (peer_id_one, peer_id_two) = ([1u8; bt::PEER_ID_LEN].into(), [0u8; bt::PEER_ID_LEN].into());
        
        // Create two handshakers
        let mut handshaker_one = BTHandshaker::<TcpStream>::new(&addr, peer_id_one).unwrap();
        let handshaker_two = BTHandshaker::<TcpStream>::new(&addr, peer_id_two).unwrap();
        
        // Open up a stream for the specified info hash on both handshakers
        let info_hash = [0u8; bt::INFO_HASH_LEN].into();
        let handshaker_one_stream = handshaker_one.stream(info_hash);
        let handshaker_two_stream = handshaker_two.stream(info_hash);
        
        // Get the address and bind port for handshaker two
        let handshaker_two_addr = SocketAddr::V4(SocketAddrV4::new(ip, handshaker_two.port()));
        
        // Connect to handshaker two from handshaker one
        handshaker_one.connect(Some(peer_id_two), info_hash, handshaker_two_addr);
        
        // Allow the handshakers to connect to each other
        thread::sleep(Duration::from_millis(100));
        
        // Should receive the peer from both handshakers
        match (handshaker_one_stream.try_recv(), handshaker_two_stream.try_recv()) {
            (Ok(_), Ok(_)) => (),
            _              => panic!("Failed to find peers on one or both handshakers...")
        };
    }
    
    #[test]
    fn positive_handshake_expiration() {
        // Assign a listen address and peer id for each handshaker
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let (peer_id_one, peer_id_two) = ([1u8; bt::PEER_ID_LEN].into(), [0u8; bt::PEER_ID_LEN].into());
        
        // Create two handshakers
        let mut handshaker_one = BTHandshaker::<TcpStream>::new(&addr, peer_id_one).unwrap();
        let handshaker_two = BTHandshaker::<TcpStream>::new(&addr, peer_id_two).unwrap();
        
        // Open up a stream for the specified info hash on both handshakers
        let info_hash = [0u8; bt::INFO_HASH_LEN].into();
        let handshaker_one_stream = handshaker_one.stream(info_hash);
        let handshaker_two_stream = handshaker_two.stream(info_hash);
        
        // Get the address and bind port for handshaker two
        let handshaker_two_addr = SocketAddr::V4(SocketAddrV4::new(ip, handshaker_two.port()));
        
        // Spin up a listener to hold on to connections from our handshaker
        let listen_addr = SocketAddr::V4(SocketAddrV4::new(ip, 48394));
        // Need to make sure the connections dont get dropped until test finishes
        let (send, recv) = mpsc::channel();
        thread::spawn(move || {
            let listener = TcpListener::bind(listen_addr).unwrap();
            
            for _ in 0..handler::MAX_CONCURRENT_CONNECTIONS {
                let connection = listener.accept().unwrap().0;
                send.send(connection).unwrap();
            }
        });
        // Wait for the thread to spin up
        thread::sleep(Duration::from_millis(100));
       
        // Saturate the maximum number of concurrent connections
        for _ in 0..handler::MAX_CONCURRENT_CONNECTIONS {
            handshaker_one.connect(None, info_hash, listen_addr);
        }
        
        // Wait for the handshakes to expire
        thread::sleep(Duration::from_millis(connection::READ_CONNECTION_TIMEOUT + 500));
        
        // Connect to handshaker two from handshaker one
        handshaker_one.connect(Some(peer_id_two), info_hash, handshaker_two_addr);
        
        // Allow the handshakers to connect to each other (additional time because event loop is currently saturated)
        thread::sleep(Duration::from_millis(500));
        
        // Should receive the peer from both handshakers
        match (handshaker_one_stream.try_recv(), handshaker_two_stream.try_recv()) {
            (Ok(_), Ok(_)) => (),
            _              => panic!("Failed to find peers on one or both handshakers...")
        };
    }
    
    #[test]
    fn positive_shutdown_on_drop() {
        // Create our handshaker addresses and ids
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let peer_id = [0u8; bt::PEER_ID_LEN].into();
        
        // Create the handshaker
        let handshaker = BTHandshaker::<TcpStream>::new(&addr, peer_id).unwrap();
        
        // Subscribe to a specific info hash
        let info_hash = [1u8; bt::INFO_HASH_LEN].into();
        let peer_recv = handshaker.stream(info_hash);
        
        // Clone the handshaker so there are two copies of it
        let handshaker_clone = handshaker.clone();
        
        // Drop one of the copies of the handshaker
        mem::drop(handshaker_clone);
        
        // Allow the event loop to process the shutdown that should NOT have been fired
        thread::sleep(Duration::from_millis(100));
        
        // Make sure that we can still receive values on our channel
        assert_eq!(peer_recv.try_recv().unwrap_err(), TryRecvError::Empty);
        
        // Drop the other copy of the handshaker so there are no more copies around
        mem::drop(handshaker);
        
        // Allow the event loop to process the shutdown that should have been fired
        thread::sleep(Duration::from_millis(100));
        
        // Assert that the channel hunp up (because of the shutdown)
        assert_eq!(peer_recv.try_recv().unwrap_err(), TryRecvError::Disconnected);
    }
}