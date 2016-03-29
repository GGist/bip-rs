//! Messaging primitives for requests.

use std::io::{self, Write};

use byteorder::{BigEndian, WriteBytesExt};
use nom::{be_u64, be_u32, IResult};

use announce::{AnnounceRequest};
use scrape::{ScrapeRequest};

// For all practical applications, this value should be hardcoded as a valid
// connection id for connection requests when operating in server mode and processing
// incoming requests.
/// Global connection id for connect requests.
pub const CONNECT_ID_PROTOCOL_ID: u64 = 0x41727101980;

/// Enumerates all types of requests that can be made to a tracker.
pub enum RequestType<'a> {
    Connect,
    Announce(AnnounceRequest<'a>),
    Scrape(ScrapeRequest<'a>)
}

impl<'a> RequestType<'a> {
    /// Create an owned version of the RequestType.
    pub fn to_owned(&self) -> RequestType<'static> {
        match self {
            &RequestType::Connect           => RequestType::Connect,
            &RequestType::Announce(ref req) => RequestType::Announce(req.to_owned()),
            &RequestType::Scrape(ref req)   => RequestType::Scrape(req.to_owned())
        }
    }
}

/// TrackerRequest which encapsulates any request sent to a tracker.
pub struct TrackerRequest<'a> {
    // Both the connection id and transaction id are techinically not unsigned according
    // to the spec, but since they are just bits we will keep them as unsigned since it
    // doesnt really make sense to not have them as unsigned (easier to generate transactions).
    connection_id:  u64,
    transaction_id: u32,
    request_type:   RequestType<'a>
}

impl<'a> TrackerRequest<'a> {
    /// Create a new TrackerRequest.
    pub fn new(conn_id: u64, trans_id: u32, req_type: RequestType<'a>) -> TrackerRequest<'a> {
        TrackerRequest{ connection_id: conn_id, transaction_id: trans_id, request_type: req_type }
    }
    
    /// Create a new TrackerRequest from the given bytes.
    pub fn from_bytes(bytes: &'a [u8]) -> IResult<&'a [u8], TrackerRequest<'a>> {
        parse_request(bytes)
    }
    
    /// Write the TrackerRequest to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        try!(writer.write_u64::<BigEndian>(self.connection_id()));
            
        match self.request_type() {
            &RequestType::Connect => {
                try!(writer.write_u32::<BigEndian>(::CONNECT_ACTION_ID));
                try!(writer.write_u32::<BigEndian>(self.transaction_id()));
            },
            &RequestType::Announce(ref req) => {
                let action_id = if req.source_ip().is_ipv4() {
                    ::ANNOUNCE_IPV4_ACTION_ID
                } else { ::ANNOUNCE_IPV6_ACTION_ID };
                try!(writer.write_u32::<BigEndian>(action_id));
                try!(writer.write_u32::<BigEndian>(self.transaction_id()));
                
                try!(req.write_bytes(writer));
            },
            &RequestType::Scrape(ref req) => {
                try!(writer.write_u32::<BigEndian>(::SCRAPE_ACTION_ID));
                try!(writer.write_u32::<BigEndian>(self.transaction_id()));
                
                try!(req.write_bytes(writer));
            }
        };
        
        Ok(())
    }
    
    /// Connection ID supplied with a request to validate the senders address.
    ///
    /// For Connect requests, this will always be equal to 0x41727101980. Therefore,
    /// you should not hand out that specific ID to peers that make a connect request.
    pub fn connection_id(&self) -> u64 {
        self.connection_id
    }
    
    /// Transaction ID supplied with a request to uniquely identify a response. 
    pub fn transaction_id(&self) -> u32 {
        self.transaction_id
    }
    
    /// Actual type of request that this TrackerRequest represents.
    pub fn request_type(&self) -> &RequestType {
        &self.request_type
    }
    
    /// Create an owned version of the TrackerRequest.
    pub fn to_owned(&self) -> TrackerRequest<'static> {
        TrackerRequest{ connection_id: self.connection_id, transaction_id: self.transaction_id,
            request_type: self.request_type.to_owned() }
    }
}

fn parse_request<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], TrackerRequest<'a>> {
    switch!(bytes, tuple!(be_u64, be_u32, be_u32),
        (CONNECT_ID_PROTOCOL_ID, ::CONNECT_ACTION_ID, tid) => value!(
            TrackerRequest::new(CONNECT_ID_PROTOCOL_ID, tid, RequestType::Connect)
        ) |
        (cid, ::ANNOUNCE_IPV4_ACTION_ID, tid) => map!(call!(AnnounceRequest::from_bytes_v4), |ann_req| {
            TrackerRequest::new(cid, tid, RequestType::Announce(ann_req))
        }) |
        (cid, ::SCRAPE_ACTION_ID, tid) => map!(call!(ScrapeRequest::from_bytes), |scr_req| {
            TrackerRequest::new(cid, tid, RequestType::Scrape(scr_req))
        }) |
        (cid, ::ANNOUNCE_IPV6_ACTION_ID, tid) => map!(call!(AnnounceRequest::from_bytes_v6), |ann_req| {
            TrackerRequest::new(cid, tid, RequestType::Announce(ann_req))
        })
    )
}