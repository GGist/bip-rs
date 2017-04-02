use std::io::{self, Cursor};
use std::net::SocketAddr;
use std::thread;

use nom::IResult;
use umio::{ELoopBuilder, Dispatcher, Provider};

use announce::AnnounceRequest;
use error::ErrorResponse;
use request::{self, TrackerRequest, RequestType};
use response::{TrackerResponse, ResponseType};
use scrape::ScrapeRequest;
use server::handler::ServerHandler;

use umio::external::Sender;

const EXPECTED_PACKET_LENGTH: usize = 1500;

/// Internal dispatch message for servers.
pub enum DispatchMessage {
    Shutdown,
}

/// Create a new background dispatcher to service requests.
pub fn create_dispatcher<H>(bind: SocketAddr, handler: H) -> io::Result<Sender<DispatchMessage>>
    where H: ServerHandler + 'static
{
    let builder = ELoopBuilder::new()
        .channel_capacity(1)
        .timer_capacity(0)
        .bind_address(bind)
        .buffer_length(EXPECTED_PACKET_LENGTH);

    let mut eloop = try!(builder.build());
    let channel = eloop.channel();

    let dispatch = ServerDispatcher::new(handler);

    thread::spawn(move || {
        eloop.run(dispatch).expect("bip_utracker: ELoop Shutdown Unexpectedly...");
    });

    Ok(channel)
}

// ----------------------------------------------------------------------------//

/// Dispatcher that executes requests asynchronously.
struct ServerDispatcher<H>
    where H: ServerHandler
{
    handler: H,
}

impl<H> ServerDispatcher<H>
    where H: ServerHandler
{
    /// Create a new ServerDispatcher.
    fn new(handler: H) -> ServerDispatcher<H> {
        ServerDispatcher { handler: handler }
    }

    /// Forward the request on to the appropriate handler method.
    fn process_request<'a, 'b>(&mut self,
                               provider: &mut Provider<'a, ServerDispatcher<H>>,
                               request: TrackerRequest<'b>,
                               addr: SocketAddr) {
        let conn_id = request.connection_id();
        let trans_id = request.transaction_id();

        match request.request_type() {
            &RequestType::Connect => {
                if conn_id == request::CONNECT_ID_PROTOCOL_ID {
                    self.forward_connect(provider, trans_id, addr);
                } // TODO: Add Logging
            }
            &RequestType::Announce(ref req) => {
                self.forward_announce(provider, trans_id, conn_id, req, addr);
            }
            &RequestType::Scrape(ref req) => {
                self.forward_scrape(provider, trans_id, conn_id, req, addr);
            }
        };
    }

    /// Forward a connect request on to the appropriate handler method.
    fn forward_connect<'a>(&mut self,
                           provider: &mut Provider<'a, ServerDispatcher<H>>,
                           trans_id: u32,
                           addr: SocketAddr) {
        self.handler.connect(addr, |result| {
            let response_type = match result {
                Ok(conn_id) => ResponseType::Connect(conn_id),
                Err(err_msg) => ResponseType::Error(ErrorResponse::new(err_msg)),
            };
            let response = TrackerResponse::new(trans_id, response_type);

            write_response(provider, response, addr);
        });
    }

    /// Forward an announce request on to the appropriate handler method.
    fn forward_announce<'a, 'b>(&mut self,
                                provider: &mut Provider<'a, ServerDispatcher<H>>,
                                trans_id: u32,
                                conn_id: u64,
                                request: &AnnounceRequest<'b>,
                                addr: SocketAddr) {
        self.handler.announce(addr, conn_id, request, |result| {
            let response_type = match result {
                Ok(response) => ResponseType::Announce(response),
                Err(err_msg) => ResponseType::Error(ErrorResponse::new(err_msg)),
            };
            let response = TrackerResponse::new(trans_id, response_type);

            write_response(provider, response, addr);
        });
    }

    /// Forward a scrape request on to the appropriate handler method.
    fn forward_scrape<'a, 'b>(&mut self,
                              provider: &mut Provider<'a, ServerDispatcher<H>>,
                              trans_id: u32,
                              conn_id: u64,
                              request: &ScrapeRequest<'b>,
                              addr: SocketAddr) {
        self.handler.scrape(addr, conn_id, request, |result| {
            let response_type = match result {
                Ok(response) => ResponseType::Scrape(response),
                Err(err_msg) => ResponseType::Error(ErrorResponse::new(err_msg)),
            };
            let response = TrackerResponse::new(trans_id, response_type);

            write_response(provider, response, addr);
        });
    }
}

/// Write the given tracker response through to the given provider.
fn write_response<'a, 'b, H>(provider: &mut Provider<'a, ServerDispatcher<H>>,
                             response: TrackerResponse<'b>,
                             addr: SocketAddr)
    where H: ServerHandler
{
    provider.outgoing(|buffer| {
        let mut cursor = Cursor::new(buffer);
        let success = response.write_bytes(&mut cursor).is_ok();

        if success {
            Some((cursor.position() as usize, addr))
        } else {
            None
        } // TODO: Add Logging
    });
}

impl<H> Dispatcher for ServerDispatcher<H>
    where H: ServerHandler
{
    type Timeout = ();
    type Message = DispatchMessage;

    fn incoming<'a>(&mut self,
                    mut provider: Provider<'a, Self>,
                    message: &[u8],
                    addr: SocketAddr) {
        let request = match TrackerRequest::from_bytes(message) {
            IResult::Done(_, req) => req,
            _ => return, // TODO: Add Logging
        };

        self.process_request(&mut provider, request, addr);
    }

    fn notify<'a>(&mut self, mut provider: Provider<'a, Self>, message: DispatchMessage) {
        match message {
            DispatchMessage::Shutdown => provider.shutdown(),
        }
    }

    fn timeout<'a>(&mut self, _: Provider<'a, Self>, _: ()) {}
}
