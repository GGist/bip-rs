use std::collections::{HashMap};
use std::collections::hash_map::{Entry};
use std::io::{self};
use std::net::{SocketAddr};
use std::sync::mpsc::{self};
use std::thread::{self};

use bip_util::bt::{PeerId, InfoHash};

use mio::{self, EventLoop, EventSet, PollOpt, Token, Handler};
use mio::tcp::{TcpListener, TcpStream};
use slab::{Slab};

use bittorrent::connection::{Connection, ConnectionResult};

#[cfg(not(test))]
pub const MAX_CONCURRENT_CONNECTIONS:  usize = 4096;

#[cfg(test)]
pub const MAX_CONCURRENT_CONNECTIONS:  usize = 256;

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

pub struct HandshakeHandler<T> where T: From<TcpStream> + Send {
    filters:          Vec<Box<Fn(&SocketAddr) -> bool + Send>>,
    listener:         (Token, TcpListener),
    protocol:         &'static str,
    peer_id:          PeerId,
    interested:       HashMap<InfoHash, Vec<mpsc::Sender<(T, PeerId)>>>,
    connections:      Slab<Connection, Token>
}

impl<T> HandshakeHandler<T> where T: From<TcpStream> + Send {
    /// Create a new HandshakeHandler.
    pub fn new(peer_id: PeerId, protocol: &'static str, listener: TcpListener, event_loop: &mut EventLoop<HandshakeHandler<T>>)
        -> io::Result<HandshakeHandler<T>> {
        let handler = HandshakeHandler{ filters: Vec::new(), listener: (Token(1), listener), protocol: protocol, peer_id: peer_id,
            interested: HashMap::new(), connections: Slab::new_starting_at(Token(2), MAX_CONCURRENT_CONNECTIONS) };
        
        try!(event_loop.register(&handler.listener.1, handler.listener.0, EventSet::readable(), PollOpt::edge() | PollOpt::oneshot()));
        
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
                    
                    let (protocol, peer_id) = (self.protocol, self.peer_id);
                    self.connections.insert_with_opt(|token| {
                        Connection::complete(stream, protocol, peer_id, token, event_loop).and_then(|c| {
                            try!(event_loop.register(c.evented(), token, c.event_set(), PollOpt::edge() | PollOpt::oneshot()));
                            
                            Ok(c)
                        }).ok()
                    });
                },
                _ => info!("bip_handshake: Error accepting a new socket...")
            }
        } else {
            // Forward the read event onto the connection
            let connection_res = if let Some(connection) = self.connections.get_mut(token) {
                let interested = &self.interested;
                connection.handle_read(event_loop, |info_hash| interested.contains_key(info_hash))
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
            connection.handle_write(event_loop)
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
            warn!("bip_handshake: An error caused event loop to shutdown...");
            event_loop.shutdown();
        } else {
            warn!("bip_handshake: A connection has been reset...");
            self.connections.remove(token);
        }
    }
    
    /// Forward the completed TcpStream to all peer receivers.
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
            event_loop.reregister(&self.listener.1, token, EventSet::readable(), PollOpt::edge() | PollOpt::oneshot()).is_err()
        } else {
            self.connections.get(token).and_then(|c| {
                event_loop.reregister(c.evented(), token, c.event_set(), PollOpt::edge() | PollOpt::oneshot()).ok()
            }).is_none()
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
    type Timeout = Token;
    type Message = HandlerTask<T>;
    
    fn notify(&mut self, event_loop: &mut EventLoop<HandshakeHandler<T>>, msg: HandlerTask<T>) {
        match msg {
            HandlerTask::ConnectPeer(expected, info_hash, addr) => {
                if is_filtered(&mut self.filters, &addr) {
                    return
                }
                
                let (protocol, peer_id) = (self.protocol, self.peer_id);
                self.connections.insert_with_opt(|token| {
                    Connection::initiate(&addr, protocol, info_hash, peer_id, expected, token).and_then(|c| {
                        try!(event_loop.register(c.evented(), token, c.event_set(), PollOpt::edge() | PollOpt::oneshot()));
                        
                        Ok(c)
                    }).ok()
                });
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
    
    fn timeout(&mut self, _: &mut EventLoop<HandshakeHandler<T>>, timeout: Token) {
        self.connections.remove(timeout);
    }
}