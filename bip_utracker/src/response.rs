//! Messaging primitives for responses.

use std::io::{self, Write};

use byteorder::{BigEndian, WriteBytesExt};
use nom::{be_u32, be_u64, IResult};

use crate::announce::AnnounceResponse;
use crate::contact::CompactPeers;
use crate::error::ErrorResponse;
use crate::scrape::ScrapeResponse;

/// Error action ids only occur in responses.
const ERROR_ACTION_ID: u32 = 3;

/// Enumerates all types of responses that can be received from a tracker.
pub enum ResponseType<'a> {
    Connect(u64),
    Announce(AnnounceResponse<'a>),
    Scrape(ScrapeResponse<'a>),
    Error(ErrorResponse<'a>),
}

impl<'a> ResponseType<'a> {
    /// Create an owned version of the ResponseType.
    pub fn to_owned(&self) -> ResponseType<'static> {
        match self {
            &ResponseType::Connect(id) => ResponseType::Connect(id),
            &ResponseType::Announce(ref res) => ResponseType::Announce(res.to_owned()),
            &ResponseType::Scrape(ref res) => ResponseType::Scrape(res.to_owned()),
            &ResponseType::Error(ref err) => ResponseType::Error(err.to_owned()),
        }
    }
}

/// TrackerResponse which encapsulates any response sent from a tracker.
pub struct TrackerResponse<'a> {
    transaction_id: u32,
    response_type: ResponseType<'a>,
}

impl<'a> TrackerResponse<'a> {
    /// Create a new TrackerResponse.
    pub fn new(trans_id: u32, res_type: ResponseType<'a>) -> TrackerResponse<'a> {
        TrackerResponse {
            transaction_id: trans_id,
            response_type: res_type,
        }
    }

    /// Create a new TrackerResponse from the given bytes.
    pub fn from_bytes(bytes: &'a [u8]) -> IResult<&'a [u8], TrackerResponse<'a>> {
        parse_response(bytes)
    }

    /// Write the TrackerResponse to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
    where
        W: Write,
    {
        match self.response_type() {
            &ResponseType::Connect(id) => {
                writer.write_u32::<BigEndian>(crate::CONNECT_ACTION_ID)?;
                writer.write_u32::<BigEndian>(self.transaction_id())?;

                writer.write_u64::<BigEndian>(id)?;
            }
            &ResponseType::Announce(ref req) => {
                let action_id = match req.peers() {
                    &CompactPeers::V4(_) => crate::ANNOUNCE_IPV4_ACTION_ID,
                    &CompactPeers::V6(_) => crate::ANNOUNCE_IPV6_ACTION_ID,
                };

                writer.write_u32::<BigEndian>(action_id)?;
                writer.write_u32::<BigEndian>(self.transaction_id())?;

                req.write_bytes(writer)?;
            }
            &ResponseType::Scrape(ref req) => {
                writer.write_u32::<BigEndian>(crate::SCRAPE_ACTION_ID)?;
                writer.write_u32::<BigEndian>(self.transaction_id())?;

                req.write_bytes(writer)?;
            }
            &ResponseType::Error(ref err) => {
                writer.write_u32::<BigEndian>(ERROR_ACTION_ID)?;
                writer.write_u32::<BigEndian>(self.transaction_id())?;

                err.write_bytes(writer)?;
            }
        };

        Ok(())
    }

    /// Transaction ID supplied with a response to uniquely identify a request.
    pub fn transaction_id(&self) -> u32 {
        self.transaction_id
    }

    /// Actual type of response that this TrackerResponse represents.
    pub fn response_type(&self) -> &ResponseType<'a> {
        &self.response_type
    }

    /// Create an owned version of the TrackerResponse.
    pub fn to_owned(&self) -> TrackerResponse<'static> {
        TrackerResponse {
            transaction_id: self.transaction_id,
            response_type: self.response_type().to_owned(),
        }
    }
}

fn parse_response<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], TrackerResponse<'a>> {
    switch!(bytes, tuple!(be_u32, be_u32),
        (crate::CONNECT_ACTION_ID, tid)  => map!(be_u64, |cid| TrackerResponse::new(tid, ResponseType::Connect(cid)) ) |
        (crate::ANNOUNCE_IPV4_ACTION_ID, tid) => map!(call!(AnnounceResponse::from_bytes_v4), |ann_res| {
            TrackerResponse::new(tid, ResponseType::Announce(ann_res))
        }) |
        (crate::SCRAPE_ACTION_ID, tid)   => map!(call!(ScrapeResponse::from_bytes), |scr_res| {
            TrackerResponse::new(tid, ResponseType::Scrape(scr_res))
        }) |
        (ERROR_ACTION_ID, tid)    => map!(call!(ErrorResponse::from_bytes), |err_res| {
            TrackerResponse::new(tid, ResponseType::Error(err_res))
        }) |
        (crate::ANNOUNCE_IPV6_ACTION_ID, tid) => map!(call!(AnnounceResponse::from_bytes_v6), |ann_req| {
            TrackerResponse::new(tid, ResponseType::Announce(ann_req))
        })
    )
}
