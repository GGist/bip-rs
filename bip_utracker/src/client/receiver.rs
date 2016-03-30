use chan::{self, Receiver, Sender};

use client::{ClientToken, ClientResponse};
use client::error::{ClientResult};

/// Responses received by a specific TrackerClient.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ClientResponses {
    recv: Receiver<(ClientToken, ClientResult<ClientResponse>)>
}

pub fn new_client_responses() -> (Sender<(ClientToken, ClientResult<ClientResponse>)>, ClientResponses) {
    let (send, recv) = chan::async();
    
    (send, ClientResponses{ recv: recv })
}

impl ClientResponses {
    /// Blocks until a value is received or the TrackerClient shuts down.
    pub fn recv(&self) -> Option<(ClientToken, ClientResult<ClientResponse>)> {
        self.recv.recv()
    }
    
    /// Iterator over the responses produced by the TrackerClient.
    pub fn iter(&self) -> ClientResponsesIter {
        ClientResponsesIter::new(self.recv.clone())
    }
}

impl IntoIterator for ClientResponses {
    type Item = (ClientToken, ClientResult<ClientResponse>);
    type IntoIter = ClientResponsesIter;
    
    fn into_iter(self) -> ClientResponsesIter {
        ClientResponsesIter::new(self.recv)
    }
}

impl<'a> IntoIterator for &'a ClientResponses {
    type Item = (ClientToken, ClientResult<ClientResponse>);
    type IntoIter = ClientResponsesIter;
    
    fn into_iter(self) -> ClientResponsesIter {
        self.iter()
    }
}

//----------------------------------------------------------------------------//

/// Iterator over responses received by a specific TrackerClient.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ClientResponsesIter {
    recv: Receiver<(ClientToken, ClientResult<ClientResponse>)>
}

impl ClientResponsesIter {
    fn new(recv: Receiver<(ClientToken, ClientResult<ClientResponse>)>) -> ClientResponsesIter {
        ClientResponsesIter{ recv: recv }
    }
}

impl Iterator for ClientResponsesIter {
    type Item = (ClientToken, ClientResult<ClientResponse>);
    
    fn next(&mut self) -> Option<(ClientToken, ClientResult<ClientResponse>)> {
        self.recv.recv()
    }
}