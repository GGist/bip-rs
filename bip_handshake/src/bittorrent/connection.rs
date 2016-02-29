use std::io::{self, Write};
use std::net::{SocketAddr};
use std::time::{Duration};

use bip_util::bt::{self, InfoHash, PeerId};
use bytes::{ByteBuf, MutByteBuf, MutBuf, Buf, Take};
use mio::{EventLoop, Token, Timeout, EventSet, TryRead, TryWrite, Handler};
use mio::tcp::{TcpStream};

use bittorrent::handler::{HandshakeHandler};

#[cfg(not(test))]
pub const READ_CONNECTION_TIMEOUT: u64 = 3000;

#[cfg(test)]
pub const READ_CONNECTION_TIMEOUT: u64 = 100;

const PROTOCOL_LEN_LEN:   usize = 1;
const RESERVED_BYTES_LEN: usize = 8;

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
#[derive(Debug)]
enum ConnectionTransition {
    ReadingHead(bool),
    Writing,
    ReadingTail(bool)
}

// External state of the connection.
pub enum ConnectionResult {
    Working,
    Finished,
    Errored
}

pub struct Connection {
    // If completing connection, InfoHash will be filled in when it is validated.
    info:      (&'static str, Option<InfoHash>, PeerId),
    state:     ConnectionState,
    // Used to create timeout that refer to where we are in the slab.
    token:     Token,
    stream:    TcpStream,
    // Set when we have created a timeout for when we are reading from the peer.
    timeout:   Option<Timeout>,
    expected:  Option<PeerId>,
    initiated: bool
}

impl Connection {
    /// Create a new handshake that is initiating the handshake with a remote peer.
    pub fn initiate(addr: &SocketAddr, protocol: &'static str, info_hash: InfoHash, peer_id: PeerId,
        expected: Option<PeerId>, token: Token) -> io::Result<Connection> {
        let stream = try!(TcpStream::connect(addr));
        
        let write_buf = generate_write_buffer(protocol, &info_hash, &peer_id);
        let conn_state = ConnectionState::Writing(write_buf);
        
        Ok(Connection{ info: (protocol, Some(info_hash), peer_id), state: conn_state, token: token,
            stream: stream, expected: expected, initiated: true, timeout: None })
    }
    
    /// Create a new handshake that is completing the handshake with a remote peer.
    pub fn complete<T>(stream: TcpStream, protocol: &'static str, peer_id: PeerId, token: Token,
        event_loop: &mut EventLoop<HandshakeHandler<T>>) -> io::Result<Connection> where T: From<TcpStream> + Send {
        let read_len = calculate_handshake_len(protocol) - 20;
        let read_buf = Take::new(ByteBuf::mut_with_capacity(read_len), read_len);
        
        let conn_state = ConnectionState::ReadingHead(read_buf);
        let mut connection = Connection{ info: (protocol, None, peer_id), state: conn_state, token: token,
            stream: stream, expected: None, initiated: false, timeout: None };
        
        try!(connection.set_read_timeout(event_loop));
        
        Ok(connection)
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
    pub fn handle_read<F, T>(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>, check_info_hash: F)
        -> ConnectionResult where F: Fn(&InfoHash) -> bool, T: From<TcpStream> + Send {
        // Clear any timeouts if they are present
        self.clear_read_timeout(event_loop);
        
        // Consume more bytes from the TcpStream
        let res_remaining = match self.state {
            ConnectionState::ReadingHead(ref mut buf) => self.stream.try_read_buf(buf).map(|_| buf.remaining()),
            ConnectionState::ReadingTail(ref mut buf) => self.stream.try_read_buf(buf).map(|_| buf.remaining()),
            _ => return ConnectionResult::Errored
        };
        
        match res_remaining {
            Ok(rem) if rem == 0 => self.advance_state(check_info_hash, event_loop),
            Ok(_)               => {
                if self.set_read_timeout(event_loop).is_ok() {
                    ConnectionResult::Working
                } else {
                    ConnectionResult::Errored
                }
            },
            Err(_)              => {
                warn!("bip_handshake: Error while reading bytes from TcpStream...");
                
                ConnectionResult::Errored
            }
        }
    }
    
    /// Handle a write event for the connection.
    pub fn handle_write<T>(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>) -> ConnectionResult
        where T: From<TcpStream> + Send {
        let res_remaining = match self.state {
            ConnectionState::Writing(ref mut buf) => self.stream.try_write_buf(buf).map(|_| buf.remaining()),
            _ => return ConnectionResult::Errored
        };
        
        let panic_on_check = |_: &InfoHash| {
            panic!("bip_handshake: Error in Connection, closure should not be checking the info hash...")
        };
        match res_remaining {
            Ok(rem) if rem == 0 => self.advance_state(panic_on_check, event_loop),
            Ok(_)               => ConnectionResult::Working,
            Err(_)              => {
                warn!("bip_handshake: Error while writing bytes to TcpStream...");
                
                ConnectionResult::Errored
            }
        }
    }
    
    /// Sets a read timeout for the connection.
    fn set_read_timeout<T>(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>) -> io::Result<()>
        where T: From<TcpStream> + Send {
        let duration = Duration::from_millis(READ_CONNECTION_TIMEOUT);
        
        let timeout = try!(event_loop.timeout(self.token, duration).map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Mio Timer Overflow")
        }));
        self.timeout = Some(timeout);
        
        Ok(())
    }
    
    /// Clears a read timeout for the connection.
    fn clear_read_timeout<T>(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>)
        where T: From<TcpStream> + Send {
        if let Some(timeout) = self.timeout.take() {
            event_loop.clear_timeout(&timeout);
        }
    }
    
    /// Compare the contents of the received handshake with our handshake.
    fn advance_state<F, T>(&mut self, check_info_hash: F, event_loop: &mut EventLoop<HandshakeHandler<T>>)
        -> ConnectionResult where F: Fn(&InfoHash) -> bool, T: From<TcpStream> + Send {
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
                let is_working = if self.are_initiator() {
                    self.state = ConnectionState::ReadingTail(Take::new(ByteBuf::mut_with_capacity(20), 20));
                    self.set_read_timeout(event_loop).is_ok()
                } else {
                    let info_hash = self.info.1.as_ref().unwrap();
                    self.state = ConnectionState::Writing(generate_write_buffer(&self.info.0, info_hash, &self.info.2));
                    true
                };
                
                // Check if the previous operations were successful
                if is_working {
                    ConnectionResult::Working
                } else {
                    ConnectionResult::Errored
                }
            }
            ConnectionTransition::Writing => {
                if self.are_initiator() {
                    let read_len = calculate_handshake_len(self.info.0) - 20;
                    self.state = ConnectionState::ReadingHead(Take::new(ByteBuf::mut_with_capacity(read_len), read_len));
                } else {
                    self.state = ConnectionState::ReadingTail(Take::new(ByteBuf::mut_with_capacity(20), 20));
                }
                
                // Try to set a read timeout since either of the next states are both reads
                if self.set_read_timeout(event_loop).is_ok() {
                    ConnectionResult::Working
                } else {
                    ConnectionResult::Errored
                }
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