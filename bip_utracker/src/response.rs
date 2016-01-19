use nom::{be_u32, IResult, be_i64};

use announce::{AnnounceResponse};
use error::{ErrorResponse};
use scrape::{ScrapeResponse};

/// Error action ids only occur in responses.
const ERROR_ACTION_ID: u32 = 3;

/// Enumerates all types of responses that can be received from a tracker.
pub enum ResponseType<'a> {
    Connect(i64),
    Announce(AnnounceResponse<'a>),
    Scrape(ScrapeResponse<'a>),
    Error(ErrorResponse<'a>)
}

/// TrackerResponse which encapsulates any response sent from a tracker.
pub struct TrackerResponse<'a> {
    transaction_id: u32,
    response_type:  ResponseType<'a>
}

impl<'a> TrackerResponse<'a> {
    /// Create a new TrackerResponse.
    pub fn new(trans_id: u32, res_type: ResponseType<'a>) -> TrackerResponse<'a> {
        TrackerResponse{ transaction_id: trans_id, response_type: res_type }
    }
    
    /// Create a new TrackerResponse from the given bytes.
    pub fn from_bytes(bytes: &'a [u8]) -> IResult<&'a [u8], TrackerResponse<'a>> {
        parse_response(bytes)
    }
    
    /// Transaction ID supplied with a response to uniquely identify a request. 
    pub fn transaction_id(&self) -> u32 {
        self.transaction_id
    }
    
    /// Actual type of response that this TrackerResponse represents.
    pub fn response_type(&self) -> &ResponseType<'a> {
        &self.response_type
    }
}

fn parse_response<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], TrackerResponse<'a>> {
    switch!(bytes, tuple!(be_u32, be_u32),
        (::CONNECT_ACTION_ID, tid)  => map!(be_i64, |cid| TrackerResponse::new(tid, ResponseType::Connect(cid)) ) |
        (::ANNOUNCE_IPV4_ACTION_ID, tid) => map!(call!(AnnounceResponse::from_bytes_v4), |ann_res| {
            TrackerResponse::new(tid, ResponseType::Announce(ann_res))
        }) |
        (::SCRAPE_ACTION_ID, tid)   => map!(call!(ScrapeResponse::from_bytes), |scr_res| {
            TrackerResponse::new(tid, ResponseType::Scrape(scr_res))
        }) |
        (ERROR_ACTION_ID, tid)    => map!(call!(ErrorResponse::from_bytes), |err_res| {
            TrackerResponse::new(tid, ResponseType::Error(err_res))
        }) |
        (::ANNOUNCE_IPV6_ACTION_ID, tid) => map!(call!(AnnounceResponse::from_bytes_v6), |ann_req| {
            TrackerResponse::new(tid, ResponseType::Announce(ann_req))
        })
    )
}