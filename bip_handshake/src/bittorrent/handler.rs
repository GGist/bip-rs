use std::collections::{HashMap};
use std::collections::hash_map::{Entry};
use std::io::{self, Write};
use std::net::{SocketAddr};
use std::sync::mpsc::{self};
use std::thread::{self};

use bip_util::bt::{self, PeerId, InfoHash};
use bytes::{ByteBuf, MutByteBuf, MutBuf, Buf, Take};
use mio::{self, EventLoop, EventSet, PollOpt, Token, Handler, TryRead, TryWrite};
use mio::tcp::{TcpListener, TcpStream};
use mio::util::{Slab};

const MAX_CONCURRENT_CONNECTIONS: usize = 1000;

const PROTOCOL_LEN_LEN:   usize = 1;
const RESERVED_BYTES_LEN: usize = 8;

pub enum HandlerTask<T> {
    /// Connect to the peer with the given information.
    ConnectPeer(Option<PeerId>, InfoHash, SocketAddr),
    /// Register a peer filter before yielding connections.
    RegisterFilter(Box<Fn(&SocketAddr) -> bool + Send>),
    /// Register a sender for peers for the given InfoHash.
    RegisterSender(InfoHash, mpsc::Sender<(T, PeerId)>),
    /// Signal the EventLoop to shutdown.
    Shutdown
}

/// Create a HandshakeHandler and return a channel to send tasks to it.
pub fn create_handshake_handler<T>(listener: TcpListener, peer_id: PeerId, protocol: &'static str)
    -> io::Result<mio::Sender<HandlerTask<T>>> where T: From<TcpStream> + Send + 'static {
    let mut event_loop = try!(EventLoop::new());
    let mut handshake_handler = try!(HandshakeHandler::new(peer_id, protocol, listener, &mut event_loop));
    
    let send_channel = event_loop.channel();
    
    thread::spawn(move || {
        if event_loop.run(&mut handshake_handler).is_err() {
            error!("bip_handshake: EventLoop run returned an error, shutting down thread...");
        }
    });
    
    Ok(send_channel)
}

//----------------------------------------------------------------------------//

// Initiator -> Transmit whole handshake -> Receive whole handshake
// Completor -> Receive handshake up to info hash -> Transmite whole handshake -> Receive peer id

/// Internal state of the connection.
#[derive(Debug)]
enum ConnectionState {
    /// Reading the first chunk of the message.
    ReadingHead(Take<MutByteBuf>),
    /// Writing all chunks of our message.
    Writing(ByteBuf),
    /// Reading the last chunk of the message.
    ReadingTail(Take<MutByteBuf>),
    /// Finished sending and receiving.
    Finished(InfoHash, PeerId)
}

/// Transitional state of the connection.
enum ConnectionTransition {
    ReadingHead(bool),
    Writing,
    ReadingTail(bool)
}

// External state of the connection.
enum ConnectionResult {
    Working,
    Finished,
    Errored
}

struct Connection {
    /// If completing connection, InfoHash will be filled in when it is validated.
    info:      (&'static str, Option<InfoHash>, PeerId),
    state:     ConnectionState,
    stream:    TcpStream,
    expected:  Option<PeerId>,
    initiated: bool
}

impl Connection {
    /// Create a new handshake that is initiating the handshake with a remote peer.
    pub fn initiate(addr: &SocketAddr, protocol: &'static str, info_hash: InfoHash, peer_id: PeerId, expected: Option<PeerId>) -> io::Result<Connection> {
        let stream = try!(TcpStream::connect(addr));
        
        let write_buf = generate_write_buffer(protocol, &info_hash, &peer_id);
        let conn_state = ConnectionState::Writing(write_buf);
        
        Ok(Connection{ info: (protocol, Some(info_hash), peer_id), state: conn_state, stream: stream, expected: expected, initiated: true })
    }
    
    /// Create a new handshake that is completing the handshake with a remote peer.
    pub fn complete(stream: TcpStream, protocol: &'static str, peer_id: PeerId, expected: Option<PeerId>) -> Connection {
        let read_len = calculate_handshake_len(protocol) - 20;
        let read_buf = Take::new(ByteBuf::mut_with_capacity(read_len), read_len);
        
        let conn_state = ConnectionState::ReadingHead(read_buf);
        
        Connection{ info: (protocol, None, peer_id), state: conn_state, stream: stream, expected: expected, initiated: false }
    }
    
    /// Destroy a connection to access the values from the handshake.
    ///
    /// Panics if the connection has not actually reached a finished state.
    pub fn destroy(self) -> (TcpStream, InfoHash, PeerId) {
        match self.state {
            ConnectionState::Finished(info_hash, peer_id) => (self.stream, info_hash, peer_id),
            _ => panic!("bip_handshake: Attempted to deconstruct a connection when it hasnt finished...")
        }
    }
    
    /// Returns the underlying evented object for the connection.
    pub fn evented(&self) -> &TcpStream {
        &self.stream
    }
    
    /// Get the EventSet corresponding to the event this connection is interested in.
    pub fn event_set(&self) -> EventSet {
        match self.state {
            ConnectionState::ReadingHead(_) => EventSet::readable(),
            ConnectionState::Writing(_)     => EventSet::writable(),
            ConnectionState::ReadingTail(_) => EventSet::readable(),
            ConnectionState::Finished(_, _) => EventSet::none()
        }
    }

    /// Handle a read event for the connection.
    ///
    /// The closure is used to validate, in the case where we are completing an existing handshake,
    /// whether or not any of our clients are interested in the given info hash, since we will not
    /// know what info hash the handshake is for until we read it.
    pub fn handle_read<F>(&mut self, check_info_hash: F) -> ConnectionResult
        where F: Fn(&InfoHash) -> bool {
        // Consume more bytes from the TcpStream
        let res_remaining = match self.state {
            ConnectionState::ReadingHead(ref mut buf) => self.stream.try_read_buf(buf).map(|_| buf.remaining()),
            ConnectionState::ReadingTail(ref mut buf) => self.stream.try_read_buf(buf).map(|_| buf.remaining()),
            _ => return ConnectionResult::Errored
        };
        
        match res_remaining {
            Ok(rem) if rem == 0 => self.advance_state(check_info_hash),
            Ok(_)               => ConnectionResult::Working,
            Err(_)              => {
                warn!("bip_handshake: Error while reading bytes from TcpStream...");
                
                ConnectionResult::Errored
            }
        }
    }
    
    /// Handle a write event for the connection.
    pub fn handle_write(&mut self) -> ConnectionResult {
        let res_remaining = match self.state {
            ConnectionState::Writing(ref mut buf) => self.stream.try_write_buf(buf).map(|_| buf.remaining()),
            _ => return ConnectionResult::Errored
        };
        
        match res_remaining {
            Ok(rem) if rem == 0 => self.advance_state(|_| panic!("bip_handshake: Error in Connection, closure should not be called")),
            Ok(_)               => ConnectionResult::Working,
            Err(_)              => {
                warn!("bip_handshake: Error while writing bytes to TcpStream...");
                
                ConnectionResult::Errored
            }
        }
    }
    
    /// Compare the contents of the received handshake with our handshake.
    fn advance_state<F>(&mut self, check_info_hash: F) -> ConnectionResult
        where F: Fn(&InfoHash) -> bool {
        // If we initiated the connection, that means our first state was writing,
        // else our first state was reading head.
        
        // (Initiated) Writing -> ReadingHead -> ReadingTail
        // (Not Initiated) ReadingHead -> Writing -> Reading
        
        // Get our transition state
        let trans_state = match self.state {
            ConnectionState::Writing(_)               => ConnectionTransition::Writing,
            ConnectionState::ReadingHead(ref mut buf) => {
                let opt_info_hash = compare_info_hash(buf.get_ref().bytes(), &self.info.1, check_info_hash);
                let protocol_matches = compare_protocol(buf.get_ref().bytes(), self.info.0);
                
                let good_transition = if opt_info_hash.is_some() && protocol_matches {
                    self.info.1 = opt_info_hash;
                    true
                } else {
                    false
                };
                
                ConnectionTransition::ReadingHead(good_transition)
            },
            ConnectionState::ReadingTail(ref mut buf) => {
                let opt_peer_id = compare_peer_id(buf.get_ref().bytes(), &self.expected);
                
                let good_transition = if opt_peer_id.is_some() {
                    self.expected = opt_peer_id;
                    true
                } else {
                    false
                };
                
                ConnectionTransition::ReadingTail(good_transition)
            },
            ConnectionState::Finished(_, _) => {
                return ConnectionResult::Finished
            }
        };
        
        // Act on our transition state
        match trans_state {
            ConnectionTransition::ReadingHead(good) if good => {
                if self.are_initiator() {
                    self.state = ConnectionState::ReadingTail(Take::new(ByteBuf::mut_with_capacity(20), 20));
                } else {
                    let info_hash = self.info.1.as_ref().unwrap();
                    self.state = ConnectionState::Writing(generate_write_buffer(&self.info.0, info_hash, &self.info.2));
                }
                
                ConnectionResult::Working
            }
            ConnectionTransition::Writing => {
                if self.are_initiator() {
                    let read_len = calculate_handshake_len(self.info.0) - 20;
                    self.state = ConnectionState::ReadingHead(Take::new(ByteBuf::mut_with_capacity(read_len), read_len));
                } else {
                    self.state = ConnectionState::ReadingTail(Take::new(ByteBuf::mut_with_capacity(20), 20));
                }
                
                ConnectionResult::Working
            },
            ConnectionTransition::ReadingTail(good) if good => {
                let info_hash = self.info.1.unwrap();
                let peer_id = self.expected.unwrap();
                
                self.state = ConnectionState::Finished(info_hash, peer_id);
                
                ConnectionResult::Finished
            },
            _ => ConnectionResult::Errored
        }
    }
    
    /// Returns true if we initiated the connection.
    fn are_initiator(&self) -> bool {
        self.initiated
    }
}

// Returns true if the protocol matches up.
fn compare_protocol(head: &[u8], protocol: &'static str) -> bool {
    let prot_len = head[0] as usize;
    
    prot_len == protocol.len() && &head[1..prot_len + 1] == protocol.as_bytes()
}

// Returns Some(InfoHash) is the info hash matches up.
fn compare_info_hash<F>(head: &[u8], opt_info_hash: &Option<InfoHash>, check_info_hash: F) -> Option<InfoHash>
    where F: Fn(&InfoHash) -> bool {
    let info_hash_offset = head.len() - bt::INFO_HASH_LEN;
    let info_hash = InfoHash::from_hash(&head[info_hash_offset..]).unwrap();
    
    if opt_info_hash.map_or(check_info_hash(&info_hash), |i| info_hash == i) {
        Some(info_hash)
    } else {
        None
    }
}

// Returns true if the PeerId matches up.
fn compare_peer_id(tail: &[u8], expected: &Option<PeerId>) -> Option<PeerId> {
    let peer_id = PeerId::from_hash(tail).unwrap();
    
    if expected.map_or(true, |p| peer_id == p) {
        Some(peer_id)
    } else {
        None
    }
}

/// Calculate the expected length of the handshake based on the protocol.
fn calculate_handshake_len(protocol: &'static str) -> usize {
    PROTOCOL_LEN_LEN + protocol.len() + RESERVED_BYTES_LEN + 20 + 20
}

/// Generate a buffer for use when writing our handshake.
fn generate_write_buffer(protocol: &'static str, info_hash: &InfoHash, peer_id: &PeerId) -> ByteBuf {
    let mut write_buf = ByteBuf::mut_with_capacity(calculate_handshake_len(protocol));
    
    write_buf.write_all(&[protocol.len() as u8]).unwrap();
    write_buf.write_all(protocol.as_bytes()).unwrap();
    write_buf.write_all(&[0u8; 8]).unwrap();
    write_buf.write_all(info_hash.as_ref()).unwrap();
    write_buf.write_all(peer_id.as_ref()).unwrap();
    
    write_buf.flip()
}

//----------------------------------------------------------------------------//

struct HandshakeHandler<T> where T: From<TcpStream> + Send {
    filters:          Vec<Box<Fn(&SocketAddr) -> bool + Send>>,
    listener:         (Token, TcpListener),
    protocol:         &'static str,
    peer_id:          PeerId,
    interested:       HashMap<InfoHash, Vec<mpsc::Sender<(T, PeerId)>>>,
    connections:      Slab<Connection>
}

impl<T> HandshakeHandler<T> where T: From<TcpStream> + Send {
    /// Create a new HandshakeHandler.
    pub fn new(peer_id: PeerId, protocol: &'static str, listener: TcpListener, event_loop: &mut EventLoop<HandshakeHandler<T>>)
        -> io::Result<HandshakeHandler<T>> {
        // Create our handler
        let handler = HandshakeHandler{ filters: Vec::new(), listener: (Token(1), listener), protocol: protocol, peer_id: peer_id,
            interested: HashMap::new(), connections: Slab::new_starting_at(Token(2), MAX_CONCURRENT_CONNECTIONS) };
        
        // Register our handler
        try!(event_loop.register(&handler.listener.1, handler.listener.0, EventSet::readable(), PollOpt::level() | PollOpt::oneshot()));
        
        // Return the handler
        Ok(handler)
    }
    
    /// Handles a read event that occured in our EventLoop.
    pub fn handle_read(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>, token: Token) {
        if self.listener.0 == token {
            self.reregister_token(event_loop, token);
            
            // Accept the connection from the listener
            match self.listener.1.accept() {
                Ok(Some((stream, addr))) => {
                    // If the peer is being filtered, just exit
                    if is_filtered(&mut self.filters[..], &addr) {
                        return
                    }
                    
                    // Create the completion connection with the remote peer
                    let connection = Connection::complete(stream, self.protocol, self.peer_id, None);
                    
                    // Add the connection to our slab
                    let opt_remove = if let Ok(token) = self.connections.insert(connection) {
                        let connection = self.connections.get(token).unwrap();
                        
                        // Register the connection with our event loop
                        event_loop.register(connection.evented(), token, connection.event_set(), PollOpt::level() | PollOpt::oneshot()).map_err(|_| token).err()
                    } else {
                        warn!("bip_handshake: Failed to add a new connection to our slab, already full...");
                        None
                    };
                    
                    // Remove the connection if the registration failed
                    if let Some(token) = opt_remove {
                        error!("bip_handshake: Failed to register connection with event loop...");
                        
                        self.handle_error(event_loop, token);
                    }
                },
                _ =>  info!("bip_handshake: Error accepting a new socket...")
            }
            
        } else {
            // Forward the read event onto the connection
            let connection_res = if let Some(connection) = self.connections.get_mut(token) {
                let interested = &self.interested;
                connection.handle_read(|info_hash| interested.contains_key(info_hash))
            } else {
                warn!("bip_handshake: Received a read event for a non existant token...");
                return
            };
            
            // Process the current status of the event
            match connection_res {
                ConnectionResult::Working  => self.reregister_token(event_loop, token),
                ConnectionResult::Errored  => self.handle_error(event_loop, token),
                ConnectionResult::Finished => {
                    let connection = self.connections.remove(token).unwrap();
                    let (stream, info_hash, peer_id) = connection.destroy();
                    
                    self.forward_connection(stream, info_hash, peer_id);
                }
            };
        }
    }
    
    /// Handles a write event that occured in our EventLoop.
    pub fn handle_write(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>, token: Token) {
        let connection_res = if let Some(connection) = self.connections.get_mut(token) {
            connection.handle_write()
        } else {
            warn!("bip_handshake: Received a write event for a non existant token...");
            return
        };
        
        // Process the current status of the event
        match connection_res {
            ConnectionResult::Working  => self.reregister_token(event_loop, token),
            ConnectionResult::Errored  => self.handle_error(event_loop, token),
            ConnectionResult::Finished => {
                let connection = self.connections.remove(token).unwrap();
                let (stream, info_hash, peer_id) = connection.destroy();
                
                self.forward_connection(stream, info_hash, peer_id);
            }
        }
    }
    
    /// Handles some error event that occured in our EventLoop.
    pub fn handle_error(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>, token: Token) {
        if self.listener.0 == token {
            event_loop.shutdown();
        } else {
            warn!("bip_handshake: A connection has been reset...");
            self.connections.remove(token);
        }
    }
    
    /// Forward the TcpStream to all peer receivers.
    fn forward_connection(&mut self, stream: TcpStream, info_hash: InfoHash, peer_id: PeerId) {
        let should_remove = if let Some(senders) = self.interested.get_mut(&info_hash) {
            senders.retain(|sender| {
               let stream_clone = if let Ok(stream) = stream.try_clone() {
                   stream
               } else {
                   warn!("bip_handshake: Failed to clone a peer connection to forward to receivers...");
                   return true
               };
                
               sender.send((T::from(stream_clone), peer_id)).is_ok()
            });
            
            senders.is_empty()
        } else {
            false
        };
        
        if should_remove {
            self.interested.remove(&info_hash);
        }
    }
    
    /// Reregister the given token with the event loop to receive its next event.
    fn reregister_token(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>, token: Token) {
        let error_occurred = if self.listener.0 == token {
            event_loop.reregister(&self.listener.1, token, EventSet::readable(), PollOpt::level() | PollOpt::oneshot()).is_err()
        } else {
            match self.connections.get(token) {
                Some(connection) => {
                    event_loop.reregister(connection.evented(), token, connection.event_set(), PollOpt::level() | PollOpt::oneshot()).is_err()
                },
                None => true
            }
        };
        
        if error_occurred {
            self.handle_error(event_loop, token);
        }
    }
}

/// Returns true if the given address is being filtered.
fn is_filtered(filters: &mut [Box<Fn(&SocketAddr) -> bool + Send>], addr: &SocketAddr) -> bool {
    let should_connect = filters.iter_mut().fold(true, |prev, filter| prev && filter(addr));
    
    !should_connect
}

impl<T> Handler for HandshakeHandler<T> where T: From<TcpStream> + Send {
    type Timeout = ();
    type Message = HandlerTask<T>;
    
    fn notify(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>, msg: HandlerTask<T>) {
        match msg {
            HandlerTask::ConnectPeer(expected, info_hash, addr) => {
                // Connect only if the peer is not being filtered
                if is_filtered(&mut self.filters, &addr) {
                    return
                }
                
                // Create the connection and add it to our list of connections
                let successful = Connection::initiate(&addr, self.protocol, info_hash, self.peer_id, expected).ok().and_then(|c| {
                    self.connections.insert(c).ok()
                }).and_then(|t| {
                    let connection = self.connections.get(t).unwrap();
                    event_loop.register(connection.evented(), t, connection.event_set(), PollOpt::level() | PollOpt::oneshot()).ok()
                }).is_some();
                
                if !successful {
                    warn!("bip_handshake: Failed to initiate a connection with a peer...");
                }
            },
            HandlerTask::RegisterFilter(filter) => {
                self.filters.push(filter);
            },
            HandlerTask::RegisterSender(info_hash, sender) => {
                match self.interested.entry(info_hash) {
                    Entry::Occupied(mut occ) => { occ.get_mut().push(sender); },
                    Entry::Vacant(vac)       => { vac.insert(vec![sender]); }
                }
            },
            HandlerTask::Shutdown => {
                event_loop.shutdown()
            }
        }
    }
    
    fn ready(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>, token: Token, events: EventSet) {
        if events.is_error() || events.is_hup() {
            self.handle_error(event_loop, token);
        } else if events.is_readable() {
            self.handle_read(event_loop, token);
        } else if events.is_writable() {
            self.handle_write(event_loop, token);
        } else {
            info!("bip_handshake: Receive an EventSet::none() event...");
        }
    }
}