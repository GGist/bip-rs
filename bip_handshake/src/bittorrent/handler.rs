use std::collections::{HashSet};
use std::io::{self, Write, Cursor, ErrorKind, Read};
use std::net::{SocketAddr};
use std::sync::{Arc, RwLock};
use std::thread::{self};
use std::marker::{PhantomData};

use bip_util::bt::{self, PeerId, InfoHash};
use mio::{EventLoop, EventSet, PollOpt, Token, Handler, EventLoopConfig, Timeout};
use mio::tcp::{TcpListener, TcpStream};
use nom::{IResult, be_u8};
use slab::{Slab, Index};

use bittorrent::{BTPeer};
use bittorrent::priority::{PriorityChannel, PriorityChannelAck};
use channel::{Channel};

const SERVER_READ_TOKEN: InnerToken = InnerToken(Token(2));
const SLAB_START_TOKEN:  InnerToken = InnerToken(Token(3));

const MAX_ELOOP_MESSAGE_CAPACITY: usize = 8192;
const MAX_CONCURRENT_CONNECTIONS: usize = 4096;
const MAX_READ_TIMEOUT_MILLIS:    u64   = 1500;

const RESERVED_BYTES_LEN: usize = 8;

pub enum Task<T> {
    Connect(Option<PeerId>, InfoHash, SocketAddr),
    Metadata(T),
    Shutdown
}

pub fn spawn_handshaker<C, T>(chan: C, listen: SocketAddr, pid: PeerId, protocol: &'static str, interest: Arc<RwLock<HashSet<InfoHash>>>)
    -> io::Result<(PriorityChannel<T>, u16)> where C: Channel<T> + 'static, T: Send + 'static + From<BTPeer> {
    let tcp_listener = try!(TcpListener::bind(&listen));
    let listen_port = try!(tcp_listener.local_addr()).port();
    
    let mut eloop_config = EventLoopConfig::new();
    eloop_config
        .notify_capacity(MAX_ELOOP_MESSAGE_CAPACITY)
        .timer_capacity(MAX_CONCURRENT_CONNECTIONS);
    let mut eloop = try!(EventLoop::configured(eloop_config));
    
    try!(eloop.register(&tcp_listener, SERVER_READ_TOKEN.0, EventSet::readable(), PollOpt::edge()));
    
    // Subtract 1 to reserve one message slot for a high priority message
    let priority_chan = PriorityChannel::new(eloop.channel(), MAX_ELOOP_MESSAGE_CAPACITY - 1);
    let mut handler = HandshakeHandler::new(tcp_listener, pid, protocol, chan, interest, priority_chan.channel_ack());
    
    thread::spawn(move || {
        eloop.run(&mut handler).unwrap()
    });
    
    Ok((priority_chan, listen_port))
}

//----------------------------------------------------------------------------//

struct InnerToken(Token);

impl Index for InnerToken {
    fn from_usize(i: usize) -> InnerToken {
        InnerToken(Token(i))
    }
    
    fn as_usize(&self) -> usize {
        self.0.as_usize()
    }
}

//----------------------------------------------------------------------------//

struct HandshakeHandler<C, T> {
    interest:     Arc<RwLock<HashSet<InfoHash>>>,
    /// Pre-Made out buffer for connections to write out to peers.
    out_buffer:   Vec<u8>,
    listener:     TcpListener,
    msg_ack:      PriorityChannelAck,
    slab:         Slab<Connection, InnerToken>,
    chan:         C,
    our_protocol: &'static str,
    _unused:      PhantomData<T>
}

impl<C, T> HandshakeHandler<C, T> where C: Channel<T>, T: Send {
    pub fn new(listener: TcpListener, pid: PeerId, protocol: &'static str, chan: C, interest: Arc<RwLock<HashSet<InfoHash>>>,
        msg_ack: PriorityChannelAck) -> HandshakeHandler<C, T> {
        let out_buffer = premade_out_buffer(protocol, [0u8; bt::INFO_HASH_LEN].into(), pid);
            
        HandshakeHandler{ interest: interest, out_buffer: out_buffer, listener: listener, msg_ack: msg_ack,
            slab: Slab::new_starting_at(SLAB_START_TOKEN, MAX_CONCURRENT_CONNECTIONS), chan: chan, our_protocol: protocol,
            _unused: PhantomData }
    }
}

impl<C, T> Handler for HandshakeHandler<C, T> where C: Channel<T>, T: Send + From<BTPeer> {
    type Timeout = InnerToken;
    type Message = Task<T>;
    
    fn ready(&mut self, event_loop: &mut EventLoop<HandshakeHandler<C, T>>, token: Token, events: EventSet) {
        // If this is a server event, start calling accept on the listener
        if token == SERVER_READ_TOKEN.0 {
            if !events.is_readable() || events.is_writable() || events.is_error() || events.is_hup() {
                panic!("bip_handshake: Mio Returned A Non-Readable Event For Listener");
            }
            
            let mut accept_result = self.listener.accept();
            while let Ok(Some((stream, _))) = accept_result {
                let (out_buffer, our_protocol) = (&self.out_buffer, self.our_protocol);
                self.slab.vacant_entry().and_then(|entry| {
                    let timeout = event_loop.timeout_ms(entry.index(), MAX_READ_TIMEOUT_MILLIS)
                        .expect("bip_handshake: Failed To Set Timeout For Read");
                    event_loop.register(&stream, entry.index().0, EventSet::readable(), PollOpt::edge() | PollOpt::oneshot())
                        .expect("bip_handshake: Failed To Register Connection Readable");
                    
                    let mut connection = Connection::new_complete(stream, out_buffer.clone(), our_protocol);
                    connection.store_timeout(timeout);
                    
                    Some(entry.insert(connection))
                });
                
                accept_result = self.listener.accept();
            }
            
            if accept_result.is_err() {
                panic!("bip_handshake: Calling Accept On A Client Connection Caused An Error")
            }
            return
        }
        
        // If this is a client event, process it as such
        let connect_state = if let Some(peer_connection) = self.slab.get_mut(InnerToken(token)) {
            let interest = &self.interest;
            if events.is_error() || events.is_hup() {
                ConnectionState::Disconnect
            } else if events.is_readable() {
                event_loop.clear_timeout(peer_connection.get_timeout());
                
                peer_connection.read(|hash| interest.read().unwrap().contains(&hash))
            } else if events.is_writable() {
                peer_connection.write()
            } else {
                ConnectionState::Disconnect
            }
        } else {
            return
        };
        
        match connect_state {
            ConnectionState::RegisterRead => {
                let connection = self.slab.get_mut(InnerToken(token)).unwrap();
                
                let timeout = event_loop.timeout_ms(InnerToken(token), MAX_READ_TIMEOUT_MILLIS)
                    .expect("bip_handshake: Failed To Set Timeout For Read");
                connection.store_timeout(timeout);
                    
                event_loop.reregister(connection.get_evented(), token, EventSet::readable(), PollOpt::edge() | PollOpt::oneshot())
                    .expect("bip_handshake: Failed To ReRegister Connection Readable");
            },
            ConnectionState::RegisterWrite => {
                let connection = self.slab.get(InnerToken(token)).unwrap();
                
                event_loop.reregister(connection.get_evented(), token, EventSet::writable(), PollOpt::edge() | PollOpt::oneshot())
                    .expect("bip_handshake: Failed To ReRegister Connection Writable");
            },
            ConnectionState::Disconnect => {
                let connection = self.slab.remove(InnerToken(token)).unwrap();
                
                event_loop.deregister(connection.get_evented())
                    .expect("bip_handshake: Failed To Deregister Connection");
            },
            ConnectionState::Completed => {
                let connection = self.slab.remove(InnerToken(token)).unwrap();
                
                event_loop.deregister(connection.get_evented())
                    .expect("bip_handshake: Failed To Deregister Connection");
                    
                let (tcp, hash, pid) = connection.destory();
                let tcp_peer = BTPeer::new(tcp, hash, pid);
                
                self.chan.send(tcp_peer.into());
            }
        }
    }
    
    fn notify(&mut self, event_loop: &mut EventLoop<HandshakeHandler<C, T>>, msg: Task<T>) {
        self.msg_ack.ack_task();
        
        match msg {
            Task::Connect(expect_pid, hash, addr) => {
                // Check if we have no interest in the given hash
                if !self.interest.read().unwrap().contains(&hash) {
                    return
                }
                
                let (out_buffer, our_protocol) = (&self.out_buffer, self.our_protocol);
                self.slab.vacant_entry().and_then(|entry| TcpStream::connect(&addr).ok().map(|stream| (stream, entry)) ).and_then(|(stream, entry)| {
                    event_loop.register(&stream, entry.index().0, EventSet::writable(), PollOpt::edge() | PollOpt::oneshot())
                        .expect("bip_handshake: Failed To Register Connection Writable");
                    
                    Some(entry.insert(Connection::new_initiate(stream, out_buffer.clone(), our_protocol, hash, expect_pid)))
                });
            },
            Task::Metadata(metadata) => {
                self.chan.send(metadata);
            },
            Task::Shutdown => {
                event_loop.shutdown();
            }
        }
    }
    
    fn timeout(&mut self, event_loop: &mut EventLoop<HandshakeHandler<C, T>>, timeout: InnerToken) {
        if let Some(conn) = self.slab.remove(timeout) {
            event_loop.deregister(conn.get_evented())
                .expect("bip_handshake: Failed To Deregister Connection");
        }
    }
}

fn premade_out_buffer(protocol: &'static str, info_hash: InfoHash, pid: PeerId) -> Vec<u8> {
    let buffer_len = 1 + protocol.len() + RESERVED_BYTES_LEN + bt::INFO_HASH_LEN + bt::PEER_ID_LEN;
    let mut buffer = Vec::with_capacity(buffer_len);
    
    buffer.write(&[protocol.len() as u8]).unwrap();
    buffer.write(protocol.as_bytes()).unwrap();
    buffer.write(&[0u8; RESERVED_BYTES_LEN]).unwrap();
    buffer.write(info_hash.as_ref()).unwrap();
    buffer.write(pid.as_ref()).unwrap();
    
    buffer
}

//----------------------------------------------------------------------------//

enum ConnectionState {
    RegisterRead,
    RegisterWrite,
    Disconnect,
    Completed
}

struct Connection {
    timeout:           Option<Timeout>,
    out_buffer:        Cursor<Vec<u8>>,
    in_buffer:         Cursor<Vec<u8>>,
    remote_stream:     TcpStream,
    expected_pid:      Option<PeerId>,
    expected_hash:     InfoHash,
    expected_protocol: &'static str,
    // Whether or not we have flipped read/write states yet
    // so we know when we can validate the complete handshake
    flipped:           bool
}

impl Connection {
    pub fn new_initiate(stream: TcpStream, mut out_buffer: Vec<u8>, expected_protocol: &'static str, expected_hash: InfoHash, expected_pid: Option<PeerId>)
        -> Connection {
        let in_buffer = Cursor::new(vec![0u8; out_buffer.len()]);
        
        rewrite_out_hash(&mut out_buffer[..], expected_protocol.len(), expected_hash);
        
        Connection{ timeout: None, out_buffer: Cursor::new(out_buffer), in_buffer: in_buffer, remote_stream: stream, expected_pid: expected_pid,
            expected_hash: expected_hash, expected_protocol: expected_protocol, flipped: false }
    }
    
    pub fn new_complete(stream: TcpStream, out_buffer: Vec<u8>, expected_protocol: &'static str) -> Connection {
        let dummy_hash = [0u8; bt::INFO_HASH_LEN].into();
        
        Connection::new_initiate(stream, out_buffer, expected_protocol, dummy_hash, None)
    }
    
    pub fn destory(self) -> (TcpStream, InfoHash, PeerId) {
        (self.remote_stream, self.expected_hash, self.expected_pid.unwrap())
    }
    
    pub fn store_timeout(&mut self, timeout: Timeout) {
        self.timeout = Some(timeout);
    }
    
    pub fn get_timeout(&self) -> Timeout {
        self.timeout.expect("bip_handshake: Tried To Access Non-Existant Timeout In Connection")
    }
    
    pub fn get_evented(&self) -> &TcpStream {
        &self.remote_stream
    }
    
    pub fn read<I>(&mut self, interested_hash: I) -> ConnectionState
        where I: Fn(InfoHash) -> bool {
        let total_buf_size = self.in_buffer.get_ref().len();
        let mut read_position = self.in_buffer.position() as usize;
        
        // Read until we receive an error, we filled our buffer, or we read zero bytes
        let mut read_result = Ok(1);
        while read_result.is_ok() && *read_result.as_ref().unwrap() != 0 && read_position != total_buf_size {
            let in_slice = &mut self.in_buffer.get_mut()[read_position..];
                
            read_result = self.remote_stream.read(in_slice);
            if let Ok(bytes_read) = read_result {
                read_position += bytes_read;
            }
        }
        self.in_buffer.set_position(read_position as u64);
        
        // Try to parse whatever part of the message we currently have (see if we need to disconnect early)
        let parse_status = {
            let in_slice = &self.in_buffer.get_mut()[..read_position];
            parse_remote_handshake(in_slice, self.expected_pid, self.expected_protocol)
        };
        // If we are flipping over to writing, that means we read first in which case we need to validate
        // the hash against the passed closure, otherwise just use the expected hash since that has already
        // been validated...and its cheaper!!!
        match (parse_status, read_result) {
            (ParseStatus::Valid(hash, pid), _) if self.flipped => {
                self.expected_pid = Some(pid);
                
                if self.expected_hash == hash {
                    ConnectionState::Completed
                } else {
                    ConnectionState::Disconnect
                }
            },
            (ParseStatus::Valid(hash, pid), _) => {
                self.expected_pid = Some(pid);
                self.expected_hash = hash;
                self.flipped = true;
                
                if interested_hash(hash) {
                    rewrite_out_hash(self.out_buffer.get_mut(), self.expected_protocol.len(), self.expected_hash);
                    
                    ConnectionState::RegisterWrite
                } else {
                    ConnectionState::Disconnect
                }
            },
            (ParseStatus::Invalid, _)       => ConnectionState::Disconnect,
            (ParseStatus::More, Ok(_))      => ConnectionState::Disconnect,
            (ParseStatus::More, Err(error)) => {
                // If we received an interrupt, we can try again, if we received a would block, need to wait again, otherwise disconnect
                match error.kind() {
                    ErrorKind::Interrupted => self.read(interested_hash),
                    ErrorKind::WouldBlock  => ConnectionState::RegisterRead,
                    _                      => ConnectionState::Disconnect
                }
            }
        }
    }
    
    pub fn write(&mut self) -> ConnectionState {
        let total_buf_size = self.out_buffer.get_ref().len();
        let mut write_position = self.out_buffer.position() as usize;
        
        // Write until we receive an error, we wrote out buffer, or we wrote zero bytes
        let mut write_result = Ok(1);
        while write_result.is_ok() && *write_result.as_ref().unwrap() != 0 && write_position != total_buf_size {
            let out_slice = &self.out_buffer.get_ref()[write_position..];
            
            write_result = self.remote_stream.write(out_slice);
            if let Ok(bytes_wrote) = write_result {
                write_position += bytes_wrote;
            }
        }
        self.out_buffer.set_position(write_position as u64);
        
        // If we didnt write whole buffer but received an Ok (where wrote == 0), then we assume the peer disconnected
        match (write_result, write_position == total_buf_size) {
            (_, true) if self.flipped => ConnectionState::Completed,
            (_, true)                 => {
                self.flipped = true;
                
                ConnectionState::RegisterRead
            },
            (Ok(_), false)            => ConnectionState::Disconnect,
            (Err(error), false)       => {
                match error.kind() {
                    ErrorKind::Interrupted => self.write(),
                    ErrorKind::WouldBlock  => ConnectionState::RegisterWrite,
                    _                      => ConnectionState::Disconnect
                }
            }
        }
    }
}

fn rewrite_out_hash(buffer: &mut [u8], prot_len: usize, hash: InfoHash) {
    let hash_offset = 1 + prot_len + RESERVED_BYTES_LEN;
    
    for (dst, src) in buffer[hash_offset..].iter_mut().zip(hash.as_ref().iter()) {
        *dst = *src;
    }
}

enum ParseStatus {
    Valid(InfoHash, PeerId),
    Invalid,
    More
}

/// Returns Some(true) if the remote handshake is valid, Some(false) if the remote handshake is invalid, or None if more bytes need to be read.
fn parse_remote_handshake(bytes: &[u8], expected_pid: Option<PeerId>, expected_protocol: &'static str) -> ParseStatus {
    let parse_result = chain!(bytes,
        _unused_prot: call!(parse_remote_protocol, expected_protocol) ~
        _unused_ext:  take!(RESERVED_BYTES_LEN) ~
        hash:         call!(parse_remote_hash) ~
        pid:          call!(parse_remote_pid, expected_pid) ,
        || { (hash, pid) }
    );
    
    match parse_result {
        IResult::Done(_, (hash, pid)) => ParseStatus::Valid(hash, pid),
        IResult::Error(_)             => ParseStatus::Invalid,
        IResult::Incomplete(_)        => ParseStatus::More
    }
}

fn parse_remote_protocol<'a>(bytes: &'a [u8], expected_protocol: &'static str) -> IResult<&'a [u8], &'a [u8]> {
    let expected_length = expected_protocol.len() as u8;
    
    switch!(bytes, map!(be_u8, |len| len == expected_length),
        true => tag!(expected_protocol.as_bytes())
    )
}

fn parse_remote_hash(bytes: &[u8]) -> IResult<&[u8], InfoHash> {
    map!(bytes, take!(bt::INFO_HASH_LEN), |hash| InfoHash::from_hash(hash).unwrap())
}

fn parse_remote_pid(bytes: &[u8], opt_expected_pid: Option<PeerId>) -> IResult<&[u8], PeerId> {
    if let Some(expected_pid) = opt_expected_pid {
        map!(bytes, tag!(expected_pid.as_ref()), |id| PeerId::from_hash(id).unwrap())
    } else {
        map!(bytes, take!(bt::PEER_ID_LEN), |id| PeerId::from_hash(id).unwrap())
    }
}