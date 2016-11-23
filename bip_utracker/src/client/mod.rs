use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bip_handshake::{Handshaker};
use bip_util::bt::{InfoHash};
use bip_util::trans::{TransactionIds, LocallyShuffledIds};
use umio::external::{Sender};

use announce::{AnnounceResponse, ClientState};
use client::dispatcher::DispatchMessage;
use client::error::ClientResult;
use scrape::ScrapeResponse;

mod dispatcher;
pub mod error;

/// Capacity of outstanding requests (assuming each request uses at most 1 timer at any time)
const DEFAULT_CAPACITY: usize = 4096;

/// Request made by the TrackerClient.
#[derive(Debug)]
pub enum ClientRequest {
    Announce(InfoHash, ClientState),
    Scrape(InfoHash),
}

/// Response metadata from a request.
#[derive(Debug)]
pub struct ClientMetadata {
    token: ClientToken,
    result: ClientResult<ClientResponse>,
}

impl ClientMetadata {
    /// Create a new ClientMetadata container.
    pub fn new(token: ClientToken, result: ClientResult<ClientResponse>) -> ClientMetadata {
        ClientMetadata {
            token: token,
            result: result,
        }
    }

    /// Access the request token corresponding to this metadata.
    pub fn token(&self) -> ClientToken {
        self.token
    }

    /// Access the result metadata for the request.
    pub fn result(&self) -> &ClientResult<ClientResponse> {
        &self.result
    }
}

/// Response received by the TrackerClient.
#[derive(Debug)]
pub enum ClientResponse {
    /// Announce response.
    Announce(AnnounceResponse<'static>),
    /// Scrape response.
    Scrape(ScrapeResponse<'static>),
}

impl ClientResponse {
    /// Optionally return a reference to the underyling AnnounceResponse.
    ///
    /// If you know that the token associated with the response was retrived
    /// from an AnnounceRequest, then unwrapping this value is guaranteed to
    /// succeed.
    pub fn announce_response(&self) -> Option<&AnnounceResponse<'static>> {
        match self {
            &ClientResponse::Announce(ref res) => Some(res),
            &ClientResponse::Scrape(_) => None,
        }
    }

    /// Optionally return a reference to the underyling ScrapeResponse.
    ///
    /// If you know that the token associated with the response was retrived
    /// from a ScrapeRequest, then unwrapping this value is guaranteed to
    /// succeed.
    pub fn scrape_response(&self) -> Option<&ScrapeResponse<'static>> {
        match self {
            &ClientResponse::Announce(_) => None,
            &ClientResponse::Scrape(ref res) => Some(res),
        }
    }
}

// ----------------------------------------------------------------------------//

/// Tracker client that executes requests asynchronously.
///
/// Client will shutdown on drop.
pub struct TrackerClient {
    send: Sender<DispatchMessage>,
    // We are in charge of incrementing this, background worker is in charge of decrementing
    limiter: RequestLimiter,
    generator: TokenGenerator,
}

impl TrackerClient {
    /// Create a new TrackerClient.
    pub fn new<H>(bind: SocketAddr, handshaker: H) -> io::Result<TrackerClient>
        where H: Handshaker + 'static,
              H::MetadataEnvelope: From<ClientMetadata>
    {
        TrackerClient::with_capacity(bind, handshaker, DEFAULT_CAPACITY)
    }

    /// Create a new TrackerClient with the given message capacity.
    ///
    /// Panics if capacity == usize::max_value().
    pub fn with_capacity<H>(bind: SocketAddr,
                            handshaker: H,
                            capacity: usize)
                            -> io::Result<TrackerClient>
        where H: Handshaker + 'static,
              H::MetadataEnvelope: From<ClientMetadata>
    {
        // Need channel capacity to be 1 more in case channel is saturated and client
        // is dropped so shutdown message can get through in the worst case
        let (chan_capacity, would_overflow) = capacity.overflowing_add(1);
        if would_overflow {
            panic!("bip_utracker: Tracker Client Capacity Must Be Less Than Max Size");
        }
        // Limit the capacity of messages (channel capacity - 1)
        let limiter = RequestLimiter::new(capacity);

        dispatcher::create_dispatcher(bind, handshaker, chan_capacity, limiter.clone())
            .map(|chan| {
                TrackerClient {
                    send: chan,
                    limiter: limiter,
                    generator: TokenGenerator::new(),
                }
            })
    }

    /// Execute an asynchronous request to the given tracker.
    ///
    /// If the maximum number of requests are currently in progress, return None.
    pub fn request(&mut self, addr: SocketAddr, request: ClientRequest) -> Option<ClientToken> {
        if self.limiter.can_initiate() {
            let token = self.generator.generate();
            self.send
                .send(DispatchMessage::Request(addr, token, request))
                .expect("bip_utracker: Failed To Send Client Request Message...");

            Some(token)
        } else {
            None
        }
    }
}

impl Drop for TrackerClient {
    fn drop(&mut self) {
        self.send
            .send(DispatchMessage::Shutdown)
            .expect("bip_utracker: Failed To Send Client Shutdown Message...");
    }
}

// ----------------------------------------------------------------------------//

/// Associates a ClientRequest with a ClientResponse.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClientToken(u32);

/// Generates tokens which double as transaction ids.
struct TokenGenerator {
    generator: LocallyShuffledIds<u32>
}

impl TokenGenerator {
    /// Create a new TokenGenerator.
    pub fn new() -> TokenGenerator {
        TokenGenerator{ generator: LocallyShuffledIds::<u32>::new() }
    }

    /// Generate a new ClientToken.
    pub fn generate(&mut self) -> ClientToken {
        ClientToken(self.generator.generate())
    }
}

// ----------------------------------------------------------------------------//

/// Limits requests based on the current number of outstanding requests.
#[derive(Clone)]
pub struct RequestLimiter {
    active: Arc<AtomicUsize>,
    capacity: usize,
}

impl RequestLimiter {
    /// Creates a new RequestLimiter.
    pub fn new(capacity: usize) -> RequestLimiter {
        RequestLimiter {
            active: Arc::new(AtomicUsize::new(0)),
            capacity: capacity,
        }
    }

    /// Acknowledges that a single request has been completed.
    pub fn acknowledge(&self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }

    /// Returns true if the request SHOULD be made, false otherwise.
    ///
    /// It is invalid to not make the request after this returns true.
    pub fn can_initiate(&self) -> bool {
        let current_active_requests = self.active.fetch_add(1, Ordering::AcqRel);

        // If the number of requests stored previously was less than the capacity,
        // then the add is considered good and a request can (SHOULD) be made.
        if current_active_requests < self.capacity {
            true
        } else {
            // Act as if the request just completed (decrement back down)
            self.acknowledge();

            false
        }
    }
}
