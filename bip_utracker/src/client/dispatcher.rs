use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::{self, Cursor};
use std::net::SocketAddr;
use std::thread;

use bip_handshake::{DiscoveryInfo, InitiateMessage, Protocol};
use bip_util::bt::PeerId;
use chrono::{DateTime, Duration};
use chrono::offset::Utc;
use futures::future::Either;
use futures::sink::{Wait, Sink};
use nom::IResult;
use rand;
use umio::{ELoopBuilder, Dispatcher, Provider};
use umio::external::{self, Timeout};

use announce::{AnnounceRequest, SourceIP, DesiredPeers};
use client::{ClientToken, ClientRequest, RequestLimiter, ClientMetadata, ClientResponse};
use client::error::{ClientResult, ClientError};
use option::AnnounceOptions;
use request::{self, TrackerRequest, RequestType};
use response::{TrackerResponse, ResponseType};
use scrape::ScrapeRequest;

const EXPECTED_PACKET_LENGTH: usize = 1500;

const CONNECTION_ID_VALID_DURATION_MILLIS: i64 = 60000;
const MAXIMUM_REQUEST_RETRANSMIT_ATTEMPTS: u64 = 8;

/// Internal dispatch timeout.
enum DispatchTimeout {
    Connect(ClientToken),
    CleanUp,
}

/// Internal dispatch message for clients.
pub enum DispatchMessage {
    Request(SocketAddr, ClientToken, ClientRequest),
    StartTimer,
    Shutdown,
}

/// Create a new background dispatcher to execute request and send responses back.
///
/// Assumes msg_capacity is less than usize::max_value().
pub fn create_dispatcher<H>(bind: SocketAddr,
                            handshaker: H,
                            msg_capacity: usize,
                            limiter: RequestLimiter)
                            -> io::Result<external::Sender<DispatchMessage>>
    where H: Sink + DiscoveryInfo + 'static + Send,
          H::SinkItem: From<Either<InitiateMessage, ClientMetadata>>
{
    // Timer capacity is plus one for the cache cleanup timer
    let builder = ELoopBuilder::new()
        .channel_capacity(msg_capacity)
        .timer_capacity(msg_capacity + 1)
        .bind_address(bind)
        .buffer_length(EXPECTED_PACKET_LENGTH);

    let mut eloop = try!(builder.build());
    let channel = eloop.channel();

    let dispatch = ClientDispatcher::new(handshaker, bind, limiter);

    thread::spawn(move || {
        eloop.run(dispatch).expect("bip_utracker: ELoop Shutdown Unexpectedly...");
    });

    channel.send(DispatchMessage::StartTimer)
        .expect("bip_utracker: ELoop Failed To Start Connect ID Timer...");

    Ok(channel)
}

// ----------------------------------------------------------------------------//

/// Dispatcher that executes requests asynchronously.
struct ClientDispatcher<H> {
    handshaker:      Wait<H>,
    pid:             PeerId,
    port:            u16,
    bound_addr:      SocketAddr,
    active_requests: HashMap<ClientToken, ConnectTimer>,
    id_cache:        ConnectIdCache,
    limiter:         RequestLimiter,
}

impl<H> ClientDispatcher<H>
    where H: Sink + DiscoveryInfo,
          H::SinkItem: From<Either<InitiateMessage, ClientMetadata>>
{
    /// Create a new ClientDispatcher.
    pub fn new(handshaker: H, bind: SocketAddr, limiter: RequestLimiter) -> ClientDispatcher<H> {
        let peer_id = handshaker.peer_id();
        let port = handshaker.port();

        ClientDispatcher {
            handshaker: handshaker.wait(),
            pid: peer_id,
            port: port,
            bound_addr: bind,
            active_requests: HashMap::new(),
            id_cache: ConnectIdCache::new(),
            limiter: limiter,
        }
    }

    /// Shutdown the current dispatcher, notifying all pending requests.
    pub fn shutdown<'a>(&mut self, provider: &mut Provider<'a, ClientDispatcher<H>>) {
        // Notify all active requests with the appropriate error
        for token_index in 0..self.active_requests.len() {
            let next_token = *self.active_requests.keys().skip(token_index).next().unwrap();

            self.notify_client(next_token, Err(ClientError::ClientShutdown));
        }
        // TODO: Clear active timeouts
        self.active_requests.clear();

        provider.shutdown();
    }

    /// Finish a request by sending the result back to the client.
    pub fn notify_client(&mut self, token: ClientToken, result: ClientResult<ClientResponse>) {
        self.handshaker.send(Either::B(ClientMetadata::new(token, result)).into())
            .unwrap_or_else(|_| panic!("NEED TO FIX"));

        self.limiter.acknowledge();
    }

    /// Process a request to be sent to the given address and associated with the given token.
    pub fn send_request<'a>(&mut self,
                            provider: &mut Provider<'a, ClientDispatcher<H>>,
                            addr: SocketAddr,
                            token: ClientToken,
                            request: ClientRequest) {
        // Check for IP version mismatch between source addr and dest addr
        match (self.bound_addr, addr) {
            (SocketAddr::V4(_), SocketAddr::V6(_)) |
            (SocketAddr::V6(_), SocketAddr::V4(_)) => {
                self.notify_client(token, Err(ClientError::IPVersionMismatch));

                return;
            }
            _ => (),
        };
        self.active_requests.insert(token, ConnectTimer::new(addr, request));

        self.process_request(provider, token, false);
    }

    /// Process a response received from some tracker and match it up against our sent requests.
    pub fn recv_response<'a, 'b>(&mut self,
                                 provider: &mut Provider<'a, ClientDispatcher<H>>,
                                 addr: SocketAddr,
                                 response: TrackerResponse<'b>) {
        let token = ClientToken(response.transaction_id());

        let conn_timer = if let Some(conn_timer) = self.active_requests.remove(&token) {
            if conn_timer.message_params().0 == addr {
                conn_timer
            } else {
                return;
            } // TODO: Add Logging (Server Receive Addr Different Than Send Addr)
        } else {
            return;
        }; // TODO: Add Logging (Server Gave Us Invalid Transaction Id)

        provider.clear_timeout(conn_timer.timeout_id()
            .expect("bip_utracker: Failed To Clear Request Timeout"));

        // Check if the response requires us to update the connection timer
        if let &ResponseType::Connect(id) = response.response_type() {
            self.id_cache.put(addr, id);

            self.active_requests.insert(token, conn_timer);
            self.process_request(provider, token, false);
        } else {
            // Match the request type against the response type and update our client
            match (conn_timer.message_params().1, response.response_type()) {
                (&ClientRequest::Announce(hash, _), &ResponseType::Announce(ref res)) => {
                    // Forward contact information on to the handshaker
                    for addr in res.peers().iter() {
                        self.handshaker.send(Either::A(InitiateMessage::new(Protocol::BitTorrent, hash, addr)).into())
                            .unwrap_or_else(|_| panic!("NEED TO FIX"));
                    }

                    self.notify_client(token, Ok(ClientResponse::Announce(res.to_owned())));
                }
                (&ClientRequest::Scrape(..), &ResponseType::Scrape(ref res)) => {
                    self.notify_client(token, Ok(ClientResponse::Scrape(res.to_owned())));
                }
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
    fn process_request<'a>(&mut self,
                           provider: &mut Provider<'a, ClientDispatcher<H>>,
                           token: ClientToken,
                           timed_out: bool) {
        let mut conn_timer = if let Some(conn_timer) = self.active_requests.remove(&token) {
            conn_timer
        } else {
            return;
        }; // TODO: Add logging

        // Resolve the duration of the current timeout to use
        let next_timeout = match conn_timer.current_timeout(timed_out) {
            Some(timeout) => timeout,
            None => {
                self.notify_client(token, Err(ClientError::MaxTimeout));

                return;
            }
        };

        let addr = conn_timer.message_params().0;
        let opt_conn_id = self.id_cache.get(conn_timer.message_params().0);

        // Resolve the type of request we need to make
        let (conn_id, request_type) = match (opt_conn_id, conn_timer.message_params().1) {
            (Some(id), &ClientRequest::Announce(hash, state)) => {
                let source_ip = match addr {
                    SocketAddr::V4(_) => SourceIP::ImpliedV4,
                    SocketAddr::V6(_) => SourceIP::ImpliedV6,
                };
                let key = rand::random::<u32>();

                (id,
                 RequestType::Announce(AnnounceRequest::new(hash,
                                                            self.pid,
                                                            state,
                                                            source_ip,
                                                            key,
                                                            DesiredPeers::Default,
                                                            self.port,
                                                            AnnounceOptions::new())))
            }
            (Some(id), &ClientRequest::Scrape(hash)) => {
                let mut scrape_request = ScrapeRequest::new();
                scrape_request.insert(hash);

                (id, RequestType::Scrape(scrape_request))
            }
            (None, _) => (request::CONNECT_ID_PROTOCOL_ID, RequestType::Connect),
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
            conn_timer.set_timeout_id(
                provider.set_timeout(DispatchTimeout::Connect(token), next_timeout)
                    .expect("bip_utracker: Failed To Set Timeout For Request"));

            self.active_requests.insert(token, conn_timer);
        }
    }
}

impl<H> Dispatcher for ClientDispatcher<H>
    where H: Sink + DiscoveryInfo,
          H::SinkItem: From<Either<InitiateMessage, ClientMetadata>>
{
    type Timeout = DispatchTimeout;
    type Message = DispatchMessage;

    fn incoming<'a>(&mut self,
                    mut provider: Provider<'a, Self>,
                    message: &[u8],
                    addr: SocketAddr) {
        let response = match TrackerResponse::from_bytes(message) {
            IResult::Done(_, rsp) => rsp,
            _ => return, // TODO: Add Logging
        };

        self.recv_response(&mut provider, addr, response);
    }

    fn notify<'a>(&mut self, mut provider: Provider<'a, Self>, message: DispatchMessage) {
        match message {
            DispatchMessage::Request(addr, token, req_type) => {
                self.send_request(&mut provider, addr, token, req_type);
            }
            DispatchMessage::StartTimer => self.timeout(provider, DispatchTimeout::CleanUp),
            DispatchMessage::Shutdown => self.shutdown(&mut provider),
        }
    }

    fn timeout<'a>(&mut self, mut provider: Provider<'a, Self>, timeout: DispatchTimeout) {
        match timeout {
            DispatchTimeout::Connect(token) => self.process_request(&mut provider, token, true),
            DispatchTimeout::CleanUp => {
                self.id_cache.clean_expired();

                provider.set_timeout(DispatchTimeout::CleanUp,
                                 CONNECTION_ID_VALID_DURATION_MILLIS as u64)
                    .expect("bip_utracker: Failed To Restart Connect Id Cleanup Timer");
            }
        };
    }
}

// ----------------------------------------------------------------------------//

/// Contains logic for making sure a valid connection id is present
/// and correctly timing out when sending requests to the server.
struct ConnectTimer {
    addr: SocketAddr,
    attempt: u64,
    request: ClientRequest,
    timeout_id: Option<Timeout>,
}

impl ConnectTimer {
    /// Create a new ConnectTimer.
    pub fn new(addr: SocketAddr, request: ClientRequest) -> ConnectTimer {
        ConnectTimer {
            addr: addr,
            attempt: 0,
            request: request,
            timeout_id: None,
        }
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

// ----------------------------------------------------------------------------//

/// Cache for storing connection ids associated with a specific server address.
struct ConnectIdCache {
    cache: HashMap<SocketAddr, (u64, DateTime<Utc>)>,
}

impl ConnectIdCache {
    /// Create a new connect id cache.
    fn new() -> ConnectIdCache {
        ConnectIdCache { cache: HashMap::new() }
    }

    /// Get an un expired connection id for the given addr.
    fn get(&mut self, addr: SocketAddr) -> Option<u64> {
        match self.cache.entry(addr) {
            Entry::Vacant(_) => None,
            Entry::Occupied(occ) => {
                let curr_time = Utc::now();
                let prev_time = occ.get().1;

                if is_expired(curr_time, prev_time) {
                    occ.remove();

                    None
                } else {
                    Some(occ.get().0)
                }
            }
        }
    }

    /// Put an un expired connection id into cache for the given addr.
    fn put(&mut self, addr: SocketAddr, connect_id: u64) {
        let curr_time = Utc::now();

        self.cache.insert(addr, (connect_id, curr_time));
    }

    /// Removes all entries that have expired.
    fn clean_expired(&mut self) {
        let curr_time = Utc::now();
        let mut curr_index = 0;

        let mut opt_curr_entry = self.cache.iter().skip(curr_index).map(|(&k, &v)| (k, v)).next();
        while let Some((addr, (_, prev_time))) = opt_curr_entry.take() {
            if is_expired(curr_time, prev_time) {
                self.cache.remove(&addr);
            }

            curr_index += 1;
            opt_curr_entry = self.cache.iter().skip(curr_index).map(|(&k, &v)| (k, v)).next();
        }
    }
}

/// Returns true if the connect id received at prev_time is now expired.
fn is_expired(curr_time: DateTime<Utc>, prev_time: DateTime<Utc>) -> bool {
    let valid_duration = Duration::milliseconds(CONNECTION_ID_VALID_DURATION_MILLIS);
    let difference = prev_time.signed_duration_since(curr_time);

    difference >= valid_duration
}
