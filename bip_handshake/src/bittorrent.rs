use std::io::{self, Write, Read};
use std::iter::{self};
use std::net::{SocketAddr, ToSocketAddrs, TcpStream, TcpListener};
use std::str::{self};
use std::sync::{Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread::{self};
use std::time::{Duration};

use bip_util::{InfoHash, PeerId};
use threadpool::{ThreadPool};

use handshaker::{Handshaker};
use infohash_map::{InfoHashMap};

const MAX_HANDSHAKE_PAYLOAD_BYTES:    usize = 1 + 255 + 8 + 20 + 20;
const MAX_THREAD_POOL_THREADS:        usize = 20;
const DEFAULT_HANDSHAKE_TIMEOUT_SECS: u64 = 20;
const DEFAULT_BTP_10_PROTOCOL:        &'static str = "BitTorrent protocol";

pub type PeerInfo<T> = (T, InfoHash, PeerId);

// TODO: Audit file for possible panics when unwrapping (ok to unwrap in external threads SOMETIMES,
// make sure we dont do it while holding a lock or it will get poisoned and affect the main thread -- fixed).

// TODO: Add logging for all unwraps.

// TODO: Source address for TcpStreams cannot really be specified at the moment, this means the listener
// and the sockets we use to connect on may have different addresses in practice.

// TODO: Make sure port returned from Handshaker is actually the port the peer should be sending data to
// which may not be the port we are listening on (nat traversal, port forwarding (upnp), etc).

/// Bittorrent handshake protocol which can accept a custom protocol specifier.
///
/// Cloning this object will make a shallow copy, so this object can be safely cloned and moved into other objects
/// that may need to manipulate it and the streams obtained from this object will be linked to those clones.
#[derive(Clone)]
pub struct BTHandshaker<T> where T: Send {
    peer_id:  PeerId,
    send:     Sender<WorkerMessage<T>>,
    shutdown: Arc<AtomicBool>,
    src_addr: SocketAddr
}

impl<T> BTHandshaker<T> where T: From<TcpStream> + Send + 'static {
    /// Initialize a bittorrent handshaker using the default bittorrent protocol.
    pub fn new<A: ToSocketAddrs>(src_addr: A, id: PeerId) -> io::Result<BTHandshaker<T>> {
        BTHandshaker::with_protocol(src_addr, id, DEFAULT_BTP_10_PROTOCOL)
    }
    
    /// Initialize a bittorrent handshaker using a custom protocol.
    ///
    /// The length of the protocol identifier MUST be able to fit within a byte.
    pub fn with_protocol<A: ToSocketAddrs>(src_addr: A, id: PeerId, protocol: &'static str)
        -> io::Result<BTHandshaker<T>> {
        let listener = try!(TcpListener::bind(src_addr));
        let listener_addr = try!(listener.local_addr());
        let shutdown = Arc::new(AtomicBool::new(false));
        
        let (send, recv) = mpsc::channel();
        
        spawn_worker(recv, protocol, id);
        spawn_listener(listener, send.clone(), shutdown.clone());
        
        Ok(BTHandshaker{ peer_id: id, send: send, shutdown: shutdown, src_addr: listener_addr })
    }
}

impl<T> Drop for BTHandshaker<T> where T: Send {
    #[allow(unused)]
    fn drop(&mut self) {
        // TODO: DONT RUN THIS IF THIS OBJECT HAS BEEN CLONED AND THIS IS NOT THE LAST REFERENCE!!!
        // Probably wrap all of the fields of this object behind another object and put it in an Arc?
    
        // Kill the worker thread
        self.send.send(WorkerMessage::Shutdown);
        
        // Kill the listener thread
        self.shutdown.store(true, Ordering::SeqCst);
        // TODO: Needs work
        TcpStream::connect(self.src_addr);
    }
}

//----------------------------------------------------------------------------//

/// Message that can be sent to a worker thread to trigger some action.
enum WorkerMessage<T> where T: Send {
    Shutdown,
    AddFilter(Box<Fn(SocketAddr) -> bool + Send>),
    AddRecipient(InfoHash, Sender<PeerInfo<T>>),
    Initiate(PeerId, InfoHash, SocketAddr),
    Complete(TcpStream)
}

/// Spawn a worker thread that acts on worker messages sent on the receiver.
fn spawn_worker<T>(recv: Receiver<WorkerMessage<T>>, protocol: &'static str, id: PeerId)
    where T: Send + 'static + From<TcpStream> {
    thread::spawn(move || {
        let thread_pool = ThreadPool::new(MAX_THREAD_POOL_THREADS);
    
        let recipients = Arc::new(InfoHashMap::new());
        let mut filters = Vec::new();
        
        for message in recv {
            match message {
                WorkerMessage::Shutdown => return,
                WorkerMessage::AddFilter(filter) => filters.push(filter),
                WorkerMessage::AddRecipient(hash, recipient) => recipients.insert(hash, recipient),
                WorkerMessage::Initiate(expected_id, hash, socket) => spawn_initiator(&thread_pool, protocol, id, &filters[..],
                    expected_id, hash, socket, &recipients),
                WorkerMessage::Complete(stream) => spawn_completor(&thread_pool, protocol, id, &filters[..], stream, &recipients)
            }
        }
    });
}

/// Returns true if the socket address should be filtered from handshaking.
fn should_filter(filters: &[Box<Fn(SocketAddr) -> bool + Send>], socket: SocketAddr) -> bool {
    filters.iter().fold(false, |prev, filter| prev || filter(socket))
}

/// Returns true if there are no active streams being stored for the given info hash.
fn inactive_streams<T>(hash: InfoHash, recipients: &Arc<InfoHashMap<Sender<PeerInfo<T>>>>) -> bool {
    !recipients.has_values(&hash)
}

/// Spawns a handshake initiator in a separate thread if the connection is not being filtered on and
/// we have active, listening streams for the given info hash associated with that address.
fn spawn_initiator<T>(thread_pool: &ThreadPool, protocol: &'static str, id: PeerId, filters: &[Box<Fn(SocketAddr) -> bool + Send>],
    expected_id: PeerId, hash: InfoHash, socket: SocketAddr,
    recipients: &Arc<InfoHashMap<Sender<PeerInfo<T>>>>) where T: Send + 'static + From<TcpStream> {
    
    if should_filter(filters, socket) || inactive_streams(hash, recipients) {
        return
    }
    
    let move_recipients = recipients.clone();
    thread_pool.execute(move || {
        initiate_handshake(protocol, id, expected_id, hash, socket, move_recipients);
    });
}

/// Spawns a handshake completor in a separate thread if the connection is not being filtered on. The handshake
/// is terminated early if the info hash given to us has no streams associated with it.
fn spawn_completor<T>(thread_pool: &ThreadPool, protocol: &'static str, id: PeerId, filters: &[Box<Fn(SocketAddr) -> bool + Send>],
    stream: TcpStream, recipients: &Arc<InfoHashMap<Sender<PeerInfo<T>>>>)
    where T: Send + 'static + From<TcpStream> {
    let should_filter = match stream.peer_addr() {
        Ok(socket) => should_filter(filters, socket),
        Err(_) if filters.is_empty() => false,
        Err(_) => true
    };
    if should_filter {
        return
    }
    
    let move_recipients = recipients.clone();
    thread_pool.execute(move || {
        complete_handshake(protocol, id, stream, move_recipients);
    });
}

/// Initiate a handshake with the given socket address using the given parameters. The handshake will be
/// terminated if the expected peer id does not match up with the peer id that we are given from the peer.
fn initiate_handshake<T>(protocol: &'static str, id: PeerId, expected_id: PeerId, hash: InfoHash, socket: SocketAddr, 
    recipients: Arc<InfoHashMap<Sender<PeerInfo<T>>>>) where T: From<TcpStream> {
    let mut stream = TcpStream::connect(socket).unwrap();
    
    write_handshake(protocol, id, hash, &mut stream);
    let response_buffer = read_handshake(&mut stream);
    
    let remote_protocol = protocol_from_handshake(&response_buffer);
    let remote_infohash = infohash_from_handshake(&response_buffer);
    let remote_peerid = peerid_from_handshake(&response_buffer);
    
    // TODO: Check if other clients employ id obfuscation leading us to drop a lot of connections.
    if remote_protocol == protocol && remote_infohash == hash && remote_peerid == expected_id {
        recipients.retain(&hash, |sender| {
            let stream_clone = stream.try_clone().unwrap();
            
            sender.send((T::from(stream_clone), remote_infohash, remote_peerid)).is_ok()
        });
    }
}

fn complete_handshake<T>(protocol: &'static str, id: PeerId, mut stream: TcpStream,
    recipients: Arc<InfoHashMap<Sender<PeerInfo<T>>>>) where T: From<TcpStream> {
    let response_buffer = read_handshake(&mut stream);
    
    let remote_protocol = protocol_from_handshake(&response_buffer);
    let remote_infohash = infohash_from_handshake(&response_buffer);
    let remote_peerid = peerid_from_handshake(&response_buffer);
    
    if remote_protocol == protocol {
        // Assume we signed up for this info hash before...
        write_handshake(protocol, id, remote_infohash, &mut stream);
        
        recipients.retain(&remote_infohash, |sender| {
            let stream_clone = stream.try_clone().unwrap();
            
            sender.send((T::from(stream_clone), remote_infohash, remote_peerid)).is_ok()
        });
    }
}

//----------------------------------------------------------------------------//

/// Write a handshake with the given parameters on the given stream.
fn write_handshake(protocol: &'static str, id: PeerId, hash: InfoHash, stream: &mut TcpStream) {
    let timeout_duration = Duration::from_secs(DEFAULT_HANDSHAKE_TIMEOUT_SECS);
    
    stream.set_write_timeout(Some(timeout_duration)).unwrap();
    
    let mut buffer = [0u8; MAX_HANDSHAKE_PAYLOAD_BYTES];
    let (protocol_len, extension_bits) = (protocol.len() as u8, [0u8; 8]);
    let write_iter = iter::once(&protocol_len)
        .chain(protocol.as_bytes())
        .chain(&extension_bits)
        .chain(hash.as_bytes())
        .chain(id.as_bytes());
    
    let write_len = write_iter.zip(buffer.iter_mut()).map( |(src, dst)| {
        *dst = *src;
    }).count();
    
    stream.write_all(&buffer[0..write_len]).unwrap();
}

/// Read a handshake from the given stream, returning a buffer of the handshake contents.
///
/// The size of the buffer may not equal the size of the handshake.
fn read_handshake(stream: &mut TcpStream) -> [u8; MAX_HANDSHAKE_PAYLOAD_BYTES] {
    let timeout_duration = Duration::from_secs(DEFAULT_HANDSHAKE_TIMEOUT_SECS);
    
    stream.set_read_timeout(Some(timeout_duration)).unwrap();
    
    let mut buffer = [0u8; MAX_HANDSHAKE_PAYLOAD_BYTES];
    stream.read_exact(&mut buffer[0..49]).unwrap();
    
    let protocol_len = buffer[0] as usize;
    let (start, end) = (49, protocol_len + 49);
    stream.read_exact(&mut buffer[start..end]).unwrap();
    
    buffer
}

/// Pull the protocol from the given handshake buffer.
fn protocol_from_handshake<'a>(buffer: &'a [u8]) -> &'a str {
    let protocol_len = buffer[0] as usize;

    let (start, end) = (1, protocol_len + 1);

    str::from_utf8(&buffer[start..end]).unwrap()
}

/// Pull the infohash from the given handshake buffer.
fn infohash_from_handshake(buffer: &[u8]) -> InfoHash {
    let protocol_len = buffer[0] as usize;
    let infohash_offset = 1 + protocol_len + 8;
    
    let (start, end) = (infohash_offset, infohash_offset + 20);
    
    InfoHash::from_bytes(&buffer[start..end]).unwrap()
}

/// Pull the peerid from the given handshake buffer.
fn peerid_from_handshake(buffer: &[u8]) -> PeerId {
    let protocol_len = buffer[0] as usize;
    let peerid_offset = 1 + protocol_len + 8 + 20;
    
    let (start, end) = (peerid_offset, peerid_offset + 20);
    
    PeerId::from_bytes(&buffer[start..end]).unwrap()
}

//----------------------------------------------------------------------------//

/// Spawn a listener thread to listen for connections on the given TcpListener.
/// Connections coming from that listener will be sent through the given channel.
fn spawn_listener<T>(listener: TcpListener, send: Sender<WorkerMessage<T>>, shutdown: Arc<AtomicBool>)
    where T: Send + 'static {
    thread::spawn(move || {
        for conn in listener.incoming() {
            if shutdown.load(Ordering::Acquire) {
                return
            }
            
            match conn {
                Ok(stream) => send.send(WorkerMessage::Complete(stream)).unwrap(),
                Err(_)     => ()
            }
        }
    });
}

//----------------------------------------------------------------------------//

impl<T> Handshaker for BTHandshaker<T> where T: Send {
    type Stream = Receiver<PeerInfo<T>>;

    fn port(&self) -> u16 {
        self.src_addr.port()
    }

    fn id(&self) -> PeerId {
        self.peer_id
    }

    fn connect(&mut self, id: PeerId, hash: InfoHash, addr: SocketAddr) {
        self.send.send(WorkerMessage::Initiate(id, hash, addr)).unwrap();
    }
    
    fn filter<F>(&mut self, process: Box<F>) where F: Fn(SocketAddr) -> bool + 'static + Send {
        self.send.send(WorkerMessage::AddFilter(process)).unwrap();
    }
    
    fn stream(&self, hash: InfoHash) -> Self::Stream {
        let (send, recv) = mpsc::channel();
        self.send.send(WorkerMessage::AddRecipient(hash, send)).unwrap();
        
        recv
    }
}