use std::collections::{HashMap};
use std::io::{self, Cursor};
use std::net::{SocketAddr};
use std::thread::{self};

use bip_handshake::{Handshaker};
use chan::{self};
use chrono::{DateTime, UTC};
use nom::{IResult};
use rand::{self};
use umio::{ELoopBuilder, Dispatcher, Provider};
use umio::external::{self, Timeout};

use announce::{AnnounceRequest, SourceIP, DesiredPeers};
use client::{ClientToken, ClientResponse, ClientRequest, RequestLimiter};
use client::error::{ClientResult, ClientError};
use option::{AnnounceOptions};
use request::{self, TrackerRequest, RequestType};
use response::{TrackerResponse, ResponseType};
use scrape::{ScrapeRequest};

const EXPECTED_PACKET_LENGTH: usize = 1500;

const CONNECTION_ID_VALID_DURATION_MILLIS: i64 = 60000;
const MAXIMUM_REQUEST_RETRANSMIT_ATTEMPTS: u64 = 8;

/// Internal dispatch message for clients.
pub enum DispatchMessage {
    Request(SocketAddr, ClientToken, ClientRequest),
    Shutdown
}

/// Create a new background dispatcher to execute request and send responses back.
pub fn create_dispatcher<H>(bind: SocketAddr, handshaker: H, msg_capacity: usize, limiter: RequestLimiter,
    rsp_send: chan::Sender<(ClientToken, ClientResult<ClientResponse>)>) -> io::Result<external::Sender<DispatchMessage>>
    where H: Handshaker + 'static {
    let builder = ELoopBuilder::new()
        .channel_capacity(msg_capacity)
        .timer_capacity(msg_capacity)
        .bind_address(bind)
        .buffer_length(EXPECTED_PACKET_LENGTH);
        
    let mut eloop = try!(builder.build());
    let channel = eloop.channel();
    
    let dispatch = ClientDispatcher::new(handshaker, bind, limiter, rsp_send);
    
    thread::spawn(move || {
        eloop.run(dispatch).expect("bip_utracker: ELoop Shutdown Unexpectedly...");
    });
    
    Ok(channel)
}

//----------------------------------------------------------------------------//

/// Dispatcher that executes requests asynchronously.
struct ClientDispatcher<H> where H: Handshaker {
    handshaker:      H,
    bound_addr:      SocketAddr,
    response_send:   chan::Sender<(ClientToken, ClientResult<ClientResponse>)>,
    active_requests: HashMap<ClientToken, ConnectTimer>,
    limiter:         RequestLimiter
}

impl<H> ClientDispatcher<H> where H: Handshaker {
    /// Create a new ClientDispatcher.
    pub fn new(handshaker: H, bind: SocketAddr, limiter: RequestLimiter, rsp_send: chan::Sender<(ClientToken, ClientResult<ClientResponse>)>)
        -> ClientDispatcher<H> {
        ClientDispatcher{ handshaker: handshaker, bound_addr: bind, response_send: rsp_send, active_requests: HashMap::new(),
            limiter: limiter }
    }
    
    /// Shutdown the current dispatcher, notifying all pending requests.
    pub fn shutdown<'a>(&mut self, provider: &mut Provider<'a, ClientDispatcher<H>>) {
        // Notify all active requests with the appropriate error
        for (&token, _) in self.active_requests.iter() {
            self.notify_client(token, Err(ClientError::ClientShutdown));
        }
        self.active_requests.clear();
        
        provider.shutdown();
    }
    
    /// Finish a request by sending the result back to the client.
    pub fn notify_client(&self, token: ClientToken, result: ClientResult<ClientResponse>) {
        self.response_send.send((token, result));
        
        self.limiter.acknowledge();
    }
    
    /// Process a request to be sent to the given address and associated with the given token.
    pub fn send_request<'a>(&mut self, provider: &mut Provider<'a, ClientDispatcher<H>>, addr: SocketAddr, token: ClientToken,
        request: ClientRequest) {
        // Check for IP version mismatch between source addr and dest addr
        match (self.bound_addr, addr) {
            (SocketAddr::V4(_), SocketAddr::V6(_)) |
            (SocketAddr::V6(_), SocketAddr::V4(_)) => {
                self.notify_client(token, Err(ClientError::IPVersionMismatch));
                
                return
            },
            _ => ()
        };
        self.active_requests.insert(token, ConnectTimer::new(addr, request));
        
        self.process_request(provider, token, false);
    }
    
    /// Process a response received from some tracker and match it up against our sent requests.
    pub fn recv_response<'a, 'b>(&mut self, provider: &mut Provider<'a, ClientDispatcher<H>>, response: TrackerResponse<'b>) {
        let token = ClientToken(response.transaction_id());
        
        let mut conn_timer = if let Some(conn_timer) = self.active_requests.remove(&token) {
            conn_timer
        } else { return }; // TODO: Add Logging
        
        provider.clear_timeout(conn_timer.timeout_id().unwrap());
        
        // Check if the response requires us to update the connection timer
        if let &ResponseType::Connect(id) = response.response_type() {
            // TODO: Check if we already had a connection id (malicious server?)
            conn_timer.set_connect_id(id);
            self.active_requests.insert(token, conn_timer);
            
            self.process_request(provider, token, false);
        } else {
            // Match the request type against the response type and update our client
            match (conn_timer.message_params().1, response.response_type()) {
                (&ClientRequest::Announce(hash, _), &ResponseType::Announce(ref res)) => {
                    // Forward contact information on to the handshaker
                    for addr in res.peers().iter() {
                        self.handshaker.connect(None, hash, addr);
                    }
                    
                    self.notify_client(token, Ok(ClientResponse::Announce(res.to_owned())));
                },
                (&ClientRequest::Scrape(..), &ResponseType::Scrape(ref res)) => {
                    self.notify_client(token, Ok(ClientResponse::Scrape(res.to_owned())));
                },
                (_, &ResponseType::Error(ref res)) => {
                    self.notify_client(token, Err(ClientError::ServerMessage(res.to_owned())));
                }
                _ => {
                    self.notify_client(token, Err(ClientError::ServerError));
                }
            }
        }
    }
    
    /// Process an existing request, either re requesting a connection id or sending the actual request again.
    ///
    /// If this call is the result of a timeout, that will decide whether to cancel the request or not.
    fn process_request<'a>(&mut self, provider: &mut Provider<'a, ClientDispatcher<H>>, token: ClientToken, timed_out: bool) {
        let mut conn_timer = if let Some(conn_timer) = self.active_requests.remove(&token) {
            conn_timer
        } else { return }; // TODO: Add logging
        
        // Resolve the duration of the current timeout to use
        let next_timeout = match conn_timer.current_timeout(timed_out) {
            Some(timeout) => timeout,
            None          => {
                self.notify_client(token, Err(ClientError::MaxTimeout));
                
                return
            }
        };
        
        // Resolve the type of request we need to make
        let (conn_id, request_type, addr) = match (conn_timer.connect_id(), conn_timer.message_params()) {
            (Some(id), (addr, &ClientRequest::Announce(hash, state))) => {
                let source_ip = match addr {
                    SocketAddr::V4(_) => SourceIP::ImpliedV4,
                    SocketAddr::V6(_) => SourceIP::ImpliedV6
                };
                let key = rand::random::<u32>();
                
                (id, RequestType::Announce(AnnounceRequest::new(hash, self.handshaker.id(), state, source_ip,
                    key, DesiredPeers::Default, self.handshaker.port(), AnnounceOptions::new())), addr)
            },
            (Some(id), (addr, &ClientRequest::Scrape(hash))) => {
                let mut scrape_request = ScrapeRequest::new();
                scrape_request.insert(hash);
                
                (id, RequestType::Scrape(scrape_request), addr)
            },
            (None, (addr, _)) => {
                (request::CONNECT_ID_PROTOCOL_ID, RequestType::Connect, addr)
            }
        };
        let tracker_request = TrackerRequest::new(conn_id, token.0, request_type);
        
        // Try to write the request out to the server
        let mut write_success = false;
        provider.outgoing(|bytes| {
            let mut writer = Cursor::new(bytes);
            write_success = tracker_request.write_bytes(&mut writer).is_ok();
            
            if write_success {
                Some((writer.position() as usize, addr))
            } else {
                None
            }
        });
        
        // If message was not sent (too long to fit) then end the request
        if !write_success {
            self.notify_client(token, Err(ClientError::MaxLength));
        } else {
            conn_timer.set_timeout_id(provider.set_timeout(token, next_timeout).unwrap());
            
            self.active_requests.insert(token, conn_timer);
        }
    }
}

impl<H> Dispatcher for ClientDispatcher<H> where H: Handshaker {
    type Timeout = ClientToken;
    type Message = DispatchMessage;
    
    fn incoming<'a>(&mut self, mut provider: Provider<'a, Self>, message: &[u8], _: SocketAddr) {
        let response = match TrackerResponse::from_bytes(message) {
            IResult::Done(_, rsp) => rsp,
            _                     => return // TODO: Add Logging
        };
        
        self.recv_response(&mut provider, response);
    }
    
    fn notify<'a>(&mut self, mut provider: Provider<'a, Self>, message: DispatchMessage) {
        match message {
            DispatchMessage::Request(addr, token, req_type) => {
                self.send_request(&mut provider, addr, token, req_type);
            },
            DispatchMessage::Shutdown => self.shutdown(&mut provider)
        }
    }
    
    fn timeout<'a>(&mut self, mut provider: Provider<'a, Self>, timeout: ClientToken) {
        self.process_request(&mut provider, timeout, true);
    }
}

//----------------------------------------------------------------------------//

/// Contains logic for making sure a valid connection id is present
/// and correctly timing out when sending requests to the server.
struct ConnectTimer {
    addr:       SocketAddr,
    attempt:    u64,
    request:    ClientRequest,
    connect_id: Option<(u64, DateTime<UTC>)>,
    timeout_id: Option<Timeout>
}

impl ConnectTimer {
    /// Create a new ConnectTimer.
    pub fn new(addr: SocketAddr, request: ClientRequest) -> ConnectTimer {
        ConnectTimer{ addr: addr, attempt: 0, request: request, connect_id: None, timeout_id: None }
    }
    
    /// Yields the current timeout value to use or None if the request should time out completely.
    pub fn current_timeout(&mut self, timed_out: bool) -> Option<u64> {
        if self.attempt == MAXIMUM_REQUEST_RETRANSMIT_ATTEMPTS {
            None
        } else {
            if timed_out {
                self.attempt += 1;
            }
            
            Some(calculate_message_timeout_millis(self.attempt))
        }
    }
    
    /// Yields the connection id if it is present and valid or None
    /// if the connection id needs to be refreshed.
    pub fn connect_id(&mut self) -> Option<u64> {
        self.connect_id = self.connect_id.and_then(|(id, created)| {
            let time_since = (UTC::now() - created).num_milliseconds();
            
            if time_since > CONNECTION_ID_VALID_DURATION_MILLIS {
                None
            } else {
                Some((id, created))
            }
        });
        
        self.connect_id.map(|info| info.0)
    }
    
    /// Sets a new connection id.
    pub fn set_connect_id(&mut self, id: u64) {
        self.connect_id = Some((id, UTC::now()));
    }
    
    /// Yields the current timeout id if one is set.
    pub fn timeout_id(&self) -> Option<Timeout> {
        self.timeout_id
    }
    
    /// Sets a new timeout id.
    pub fn set_timeout_id(&mut self, id: Timeout) {
        self.timeout_id = Some(id);
    }
    
    /// Yields the message parameters for the current connection.
    pub fn message_params(&self) -> (SocketAddr, &ClientRequest) {
        (self.addr, &self.request)
    }
}

/// Calculates the timeout for the request given the attempt count.
fn calculate_message_timeout_millis(attempt: u64) -> u64 {
    (15 * 2u64.pow(attempt as u32)) * 1000
}