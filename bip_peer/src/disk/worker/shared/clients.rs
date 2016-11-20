use std::sync::{RwLock, Mutex};
use std::collections::HashMap;

use bip_util::send::TrySender;

use disk::ODiskMessage;
use token::Token;

/// Struct for holding channels to send messages back to clients.
pub struct Clients {
    clients: RwLock<HashMap<Token, Mutex<Box<TrySender<ODiskMessage>>>>>
}

impl Clients {
    /// Create a new Clients struct.
    pub fn new() -> Clients {
        Clients{ clients: RwLock::new(HashMap::new()) }
    }

    /// Add the given client to the mapping of clients with the given token.
    pub fn add(&self, client_token: Token, client: Box<TrySender<ODiskMessage>>) {
        let mut map = self.clients.write().expect("bip_peer: Clients::add Poisoned Lock Detected");

        if map.insert(client_token, Mutex::new(client)).is_some() {
            panic!("bip_peer: Clients::add Token Already In Map");
        }
    }

    /// Remove the client mapped to the given token.
    pub fn remove(&self, client_token: Token) {
        let mut map = self.clients.write().expect("bip_peer: Clients::remove Poisoned Lock Detected");

        if map.remove(&client_token).is_none() {
            panic!("bip_peer: Clients::remove Token Not Found In Map");
        }
    }

    /// Send a message to the client associated with the given token.
    pub fn message(&self, client_token: Token, message: ODiskMessage) {
        let map = self.clients.read().expect("bip_peer: Clients::message Poisoned Lock Detected");
        
        let sender = map.get(&client_token).expect("bip_peer: Clients::message Token Not Found In Map");
        let unlocked_sender = sender.lock().expect("bip_peer: Clients::message Poisoned Mutex Detected");

        if unlocked_sender.try_send(message).is_some() {
            panic!("bip_peer: Clients::message Failed To Send Message To Client");
        }
    }
}