use std::io::{self, Error, ErrorKind};
use std::net::{SocketAddr};
use std::sync::mpsc::{self};

use bip_util::bt::{InfoHash, PeerId};
use mio::{self};
use mio::tcp::{TcpListener, TcpStream};

use handshaker::{Handshaker};
use bittorrent::handler::{HandlerTask};

mod handler;

const MAX_PROTOCOL_LEN: usize        = 255;
const BTP_10_PROTOCOL:  &'static str = "BitTorrent protocol";

/// Handshaker that uses the bittorrent handshake protocol.
pub struct BTHandshaker<T> where T: Send {
    send:    mio::Sender<HandlerTask<T>>,
    port:    u16,
    peer_id: PeerId
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
            
            Ok(BTHandshaker{ send: send, port: listen_port, peer_id: peer_id })
        }
    }
}

impl<T> Handshaker for BTHandshaker<T> where T: Send {
    type Stream = mpsc::Receiver<(T, PeerId)>;
    
    fn id(&self) -> PeerId {
        self.peer_id
    }
    
    fn port(&self) -> u16 {
        self.port
    }
    
    fn connect(&mut self, expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr) {
        self.send.send(HandlerTask::ConnectPeer(expected, hash, addr));
    }
    
    fn filter<F>(&mut self, process: Box<F>) where F: Fn(&SocketAddr) -> bool + Send + 'static {
        self.send.send(HandlerTask::RegisterFilter(process));
    }
    
    fn stream(&self, hash: InfoHash) -> mpsc::Receiver<(T, PeerId)> {
        let (send, recv) = mpsc::channel();
        self.send.send(HandlerTask::RegisterSender(hash, send));
        
        recv
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Write};
    use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
    use std::thread::{self};
    
    use bip_util::bt::{self, PeerId};
    use mio::tcp::{TcpStream};
    
    use bittorrent::{BTHandshaker};
    use handshaker::{Handshaker};
    
    #[test]
    fn positive_make_conenction() {
        // Assign a listen address and peer id for each handshaker
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let (addr_one, addr_two) = (SocketAddr::V4(SocketAddrV4::new(ip, 0)), SocketAddr::V4(SocketAddrV4::new(ip, 0)));
        let (peer_id_one, peer_id_two) = ([1u8; bt::PEER_ID_LEN].into(), [0u8; bt::PEER_ID_LEN].into());
        
        // Create two handshakers
        let mut handshaker_one = BTHandshaker::<TcpStream>::new(&addr_one, peer_id_one).unwrap();
        let handshaker_two = BTHandshaker::<TcpStream>::new(&addr_two, peer_id_two).unwrap();
        
        // Open up a stream for the specified info hash on both handshakers
        let info_hash = [0u8; bt::INFO_HASH_LEN].into();
        let handshaker_one_stream = handshaker_one.stream(info_hash);
        let handshaker_two_stream = handshaker_two.stream(info_hash);
        
        // Get the address and bind port for handshaker two
        let handshaker_two_addr = SocketAddr::V4(SocketAddrV4::new(ip, handshaker_two.port()));
        
        // Connect to handshaker two from handshaker one
        handshaker_one.connect(Some(peer_id_two), info_hash, handshaker_two_addr);
        
        let tcp_one = handshaker_one_stream.recv().unwrap();
        let tcp_two = handshaker_two_stream.recv().unwrap();
    }
}