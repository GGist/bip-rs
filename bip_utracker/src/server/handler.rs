use std::net::SocketAddr;

use announce::{AnnounceRequest, AnnounceResponse};
use scrape::{ScrapeRequest, ScrapeResponse};

/// Result type for a ServerHandler.
///
/// Either the response T or an error message.
pub type ServerResult<'a, T> = Result<T, &'a str>;

/// Trait for providing a TrackerServer with methods to service TrackerReqeusts.
pub trait ServerHandler: Send {
    /// Service a connection id request from the given address.
    ///
    /// If the result callback is not called, no response will be sent.
    fn connect<R>(&mut self, addr: SocketAddr, result: R)
        where R: for<'a> FnOnce(ServerResult<'a, u64>);

    /// Service an announce request with the given connect id.
    ///
    /// If the result callback is not called, no response will be sent.
    fn announce<'b, R>(&mut self,
                       addr: SocketAddr,
                       id: u64,
                       req: &AnnounceRequest<'b>,
                       result: R)
        where R: for<'a> FnOnce(ServerResult<'a, AnnounceResponse<'a>>);

    /// Service a scrape request with the given connect id.
    ///
    /// If the result callback is not called, no response will be sent.
    fn scrape<'b, R>(&mut self, addr: SocketAddr, id: u64, req: &ScrapeRequest<'b>, result: R)
        where R: for<'a> FnOnce(ServerResult<'a, ScrapeResponse<'a>>);
}
