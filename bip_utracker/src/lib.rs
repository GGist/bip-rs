//! Library for parsing and writing UDP tracker messages.
//!
//! Includes a default implementation of a bittorrent UDP tracker client
//! and a customizable trait based implementation of a bittorrent UDP tracker
//! server.

extern crate bip_handshake;
extern crate bip_util;
extern crate byteorder;
extern crate chrono;
extern crate futures;
#[macro_use]
extern crate nom;
extern crate rand;
extern crate umio;

// Action ids used in both requests and responses.
const CONNECT_ACTION_ID: u32 = 0;
const ANNOUNCE_IPV4_ACTION_ID: u32 = 1;
const SCRAPE_ACTION_ID: u32 = 2;
const ANNOUNCE_IPV6_ACTION_ID: u32 = 4;

pub mod request;
pub mod response;

pub mod announce;
pub mod contact;
pub mod error;
pub mod option;
pub mod scrape;

mod client;
mod server;

pub use client::{TrackerClient, ClientRequest, ClientResponse, ClientToken, ClientMetadata};
pub use client::error::{ClientResult, ClientError};

pub use server::TrackerServer;
pub use server::handler::{ServerResult, ServerHandler};

pub use bip_handshake::Handshaker;
pub use bip_util::bt::{InfoHash, PeerId};
