use std::sync::{RwLock, Mutex};
use std::collections::HashMap;

use bip_util::send::TrySender;

use disk::ODiskMessage;
use token::Token;

/// Struct for holding channels to send messages back to clients as well as client metadata.
pub struct Clients<MD> {
    clients:  RwLock<HashMap<Token, Mutex<ClientData<MD>>>>
}

/// Inner struct for holding client information.
struct ClientData<MD> {
    sender:   Box<TrySender<ODiskMessage>>,
    metadata: HashMap<Token, MD>
}

impl<MD> ClientData<MD> {
    fn new(sender: Box<TrySender<ODiskMessage>>) -> ClientData<MD> {
        ClientData{ sender: sender, metadata: HashMap::new() }
    }
}

impl<MD> Clients<MD> {
    /// Create a new Clients struct.
    pub fn new() -> Clients<MD> {
        Clients{ clients: RwLock::new(HashMap::new()) }
    }

    /// Add the given client to the mapping of clients with the given token.
    pub fn add_client(&self, client_token: Token, client: Box<TrySender<ODiskMessage>>) {
        self.run_with_clients_map_mut(|clients_map| {
            let clients_entry = Mutex::new(ClientData::new(client));

            if clients_map.insert(client_token, clients_entry).is_some() {
                panic!("bip_peer: Clients::add_client Token Already In Map");
            }
        });
    }

    /// Remove the client mapped to the given token.
    ///
    /// This will implicitly remove all metadata associated with the client as well.
    pub fn remove_client(&self, client_token: Token) {
        self.run_with_clients_map_mut(|clients_map| {
            if clients_map.remove(&client_token).is_none() {
                panic!("bip_peer: Clients::remove_client Token Not Found In Map");
            }
        });
    }

    /// Send a message to the client associated with the given token.
    pub fn message_client(&self, client_token: Token, message: ODiskMessage) {
        self.run_with_client(client_token, |client_data| {
            if client_data.sender.try_send(message).is_some() {
                panic!("bip_peer: Clients::message_client Failed To Send Message To Client");
            }
        });
    }

    /// Associate metadata with the given client token.
    pub fn associate_metadata(&self, client_token: Token, request_token: Token, metadata: MD) {
        self.run_with_client(client_token, |client_data| {
            if client_data.metadata.insert(request_token, metadata).is_some() {
                panic!("bip_peer: Clients::associate_metadata Detected Metadata With Same Request Token");
            }
        });
    }

    /// Remove metadata associated with the given client.
    pub fn remove_metadata(&self, client_token: Token, request_token: Token) -> MD {
        self.run_with_client(client_token, |client_data| {
            return client_data.metadata.remove(&request_token)
                .expect("bip_peer: Clients::remove_metadata Found No Metadata For Request Token");
        })
    }

    /// Run the given function with read access to the clients map.
    fn run_with_clients_map<F, R>(&self, operation: F) -> R
        where F: FnOnce(&HashMap<Token, Mutex<ClientData<MD>>>) -> R {
        let map = self.clients.write()
            .expect("bip_peer: Clients::run_with_clients_map Poisoned Map Lock Detected");

        operation(&map)
    }

    /// Run the given function with write access to the clients map.
    fn run_with_clients_map_mut<F, R>(&self, operation: F) -> R
        where F: FnOnce(&mut HashMap<Token, Mutex<ClientData<MD>>>) -> R {
        let mut map = self.clients.write()
            .expect("bip_peer: Clients::run_with_clients_map_mut Poisoned Map Lock Detected");

        operation(&mut map)
    }

    /// Run the given function with write access to the clients data.
    fn run_with_client<F, R>(&self, client_token: Token, operation: F) -> R
        where F: FnOnce(&mut ClientData<MD>) -> R {
        self.run_with_clients_map(|clients_map| {
            let locked_client = clients_map.get(&client_token)
                .expect("bip_peer: Clients::run_with_client Failed To Find Client Token");
            let mut unlocked_client = locked_client.lock()
                .expect("bip_peer: Clients::run_with_client Poisoned Client Lock Detected");

            operation(&mut unlocked_client)
        })
    }
}