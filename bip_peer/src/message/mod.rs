//! Serializable and deserializable protocol messages.
#![allow(unused)]

use std::io::{self, Write};

use protocol::PeerProtocol;

use byteorder::{WriteBytesExt, BigEndian};
use nom::{IResult, be_u32, be_u8};

const KEEP_ALIVE_MESSAGE_LEN:   u32 = 0;
const CHOKE_MESSAGE_LEN:        u32 = 1;
const UNCHOKE_MESSAGE_LEN:      u32 = 1;
const INTERESTED_MESSAGE_LEN:   u32 = 1;
const UNINTERESTED_MESSAGE_LEN: u32 = 1;
const HAVE_MESSAGE_LEN:         u32 = 5;
const REQUEST_MESSAGE_LEN:      u32 = 13;
const CANCEL_MESSAGE_LEN:       u32 = 13;

const CHOKE_MESSAGE_ID:        u8 = 0;
const UNCHOKE_MESSAGE_ID:      u8 = 1;
const INTERESTED_MESSAGE_ID:   u8 = 2;
const UNINTERESTED_MESSAGE_ID: u8 = 3;
const HAVE_MESSAGE_ID:         u8 = 4;
const BITFIELD_MESSAGE_ID:     u8 = 5;
const REQUEST_MESSAGE_ID:      u8 = 6;
const PIECE_MESSAGE_ID:        u8 = 7;
const CANCEL_MESSAGE_ID:       u8 = 8;

const MESSAGE_LENGTH_LEN_BYTES: usize = 4;

mod bits_extension;
mod standard;

pub use message::bits_extension::{BitsExtensionMessage, PortMessage};
pub use message::standard::{HaveMessage, BitFieldMessage, RequestMessage, PieceMessage, CancelMessage};

/// Enumeration of all `PeerWireMessage` types.
pub enum PeerWireProtocolMessage<P> where P: PeerProtocol {
    /// Message to keep the connection alive.
    KeepAlive,
    /// Message to tell a peer we will not be responding to their requests.
    ///
    /// Peers may wish to send *Interested and/or KeepAlive messages.
    Choke,
    /// Message to tell a peer we will now be responding to their requests.
    UnChoke,
    /// Message to tell a peer we are interested in downloading pieces from them.
    Interested,
    /// Message to tell a peer we are not interested in downloading pieces from them.
    UnInterested,
    /// Message to tell a peer we have some (validated) piece.
    Have(HaveMessage),
    /// Message to effectively send multiple HaveMessages in a single message.
    ///
    /// This message is only valid when the connection is initiated with the peer.
    BitField(BitFieldMessage),
    /// Message to request a block from a peer.
    Request(RequestMessage),
    /// Message from a peer containing a block.
    Piece(PieceMessage),
    /// Message to cancel a block request from a peer.
    Cancel(CancelMessage),
    /// Extension messages which are activated via the `ExtensionBits` as part of the handshake.
    BitsExtension(BitsExtensionMessage),
    /// Extension messages which are activated via the Extension Protocol.
    ///
    /// In reality, this can be any type that implements `ProtocolMessage` if, for example,
    /// you are running a private swarm where you know all nodes support a given message(s).
    ProtExtension(P::ProtocolMessage)
}

impl<P> PeerWireProtocolMessage<P> where P: PeerProtocol {
    pub fn parse_bytes<'a>(bytes: &'a [u8], ext_protocol: &mut P) -> IResult<&'a [u8], PeerWireProtocolMessage<P>> {
        parse_message(bytes, ext_protocol)
    }

    pub fn write_bytes<W>(&self, mut writer: W, ext_protocol: &mut P) -> io::Result<()>
        where W: Write
    {
        match self {
            &PeerWireProtocolMessage::KeepAlive              => write_length_id_pair(writer, KEEP_ALIVE_MESSAGE_LEN, None),
            &PeerWireProtocolMessage::Choke                  => write_length_id_pair(writer, CHOKE_MESSAGE_LEN, Some(CHOKE_MESSAGE_ID)),
            &PeerWireProtocolMessage::UnChoke                => write_length_id_pair(writer, UNCHOKE_MESSAGE_LEN, Some(UNCHOKE_MESSAGE_ID)),
            &PeerWireProtocolMessage::Interested             => write_length_id_pair(writer, INTERESTED_MESSAGE_LEN, Some(INTERESTED_MESSAGE_ID)),
            &PeerWireProtocolMessage::UnInterested           => write_length_id_pair(writer, UNINTERESTED_MESSAGE_LEN, Some(UNINTERESTED_MESSAGE_ID)),
            &PeerWireProtocolMessage::Have(ref msg)          => msg.write_bytes(writer),
            &PeerWireProtocolMessage::BitField(ref msg)      => msg.write_bytes(writer),
            &PeerWireProtocolMessage::Request(ref msg)       => msg.write_bytes(writer),
            &PeerWireProtocolMessage::Piece(ref msg)         => msg.write_bytes(writer),
            &PeerWireProtocolMessage::Cancel(ref msg)        => msg.write_bytes(writer),
            &PeerWireProtocolMessage::BitsExtension(ref ext) => ext.write_bytes(writer),
            &PeerWireProtocolMessage::ProtExtension(ref ext) => ext_protocol.write_bytes(ext, writer)
        }
    }
}

/// Write a length and optional id out to the given writer.
fn write_length_id_pair<W>(mut writer: W, length: u32, opt_id: Option<u8>) -> io::Result<()>
    where W: Write
{
    try!(writer.write_u32::<BigEndian>(length));

    if let Some(id) = opt_id {
        writer.write_u8(id)
    } else {
        Ok(())
    }
}

/// Parse the length portion of a message.
///
/// Panics if parsing failed for any reason.
fn parse_message_length(bytes: &[u8]) -> usize {
    if let IResult::Done(_, len) = be_u32(bytes) {
        u32_to_usize(len)
    } else {
        panic!("bip_peer: Message Length Was Less Than 4 Bytes")
    }
}

/// Panics if the conversion from a u32 to usize is not valid.
fn u32_to_usize(value: u32) -> usize {
    if value as usize as u32 != value {
        panic!("bip_peer: Cannot Convert u32 To usize, usize Is Less Than 32-Bits")
    }

    value as usize
}

// Since these messages may come over a stream oriented protocol, if a message is incomplete
// the number of bytes needed will be returned. However, that number of bytes is on a per parser
// basis. If possible, we should return the number of bytes needed for the rest of the WHOLE message.
// This allows clients to only re invoke the parser when it knows it has enough of the data.
fn parse_message<'a, P>(bytes: &'a [u8], ext_protocol: &mut P) -> IResult<&'a [u8], PeerWireProtocolMessage<P>>
    where P: PeerProtocol {
    // Attempt to parse a built in message type, otherwise, see if it is an extension type.
    alt!(bytes,
         switch!(tuple!(be_u32, opt!(be_u8)),
            (KEEP_ALIVE_MESSAGE_LEN, None) => value!(
                PeerWireProtocolMessage::KeepAlive
            ) |
            (CHOKE_MESSAGE_LEN, Some(CHOKE_MESSAGE_ID)) => value!(
                PeerWireProtocolMessage::Choke
            ) |
            (UNCHOKE_MESSAGE_LEN, Some(UNCHOKE_MESSAGE_ID)) => value!(
                PeerWireProtocolMessage::UnChoke
            ) |
            (INTERESTED_MESSAGE_LEN, Some(INTERESTED_MESSAGE_ID)) => value!(
                PeerWireProtocolMessage::Interested
            ) |
            (UNINTERESTED_MESSAGE_LEN, Some(UNINTERESTED_MESSAGE_ID)) => value!(
                PeerWireProtocolMessage::UnInterested
            ) |
            (HAVE_MESSAGE_LEN, Some(HAVE_MESSAGE_ID)) => map!(
                call!(HaveMessage::parse_bytes),
                |have| PeerWireProtocolMessage::Have(have)
            ) |
            (message_len, Some(BITFIELD_MESSAGE_ID)) => map!(
                call!(BitFieldMessage::parse_bytes, message_len - 1),
                |bitfield| PeerWireProtocolMessage::BitField(bitfield)
            ) |
            (REQUEST_MESSAGE_LEN, Some(REQUEST_MESSAGE_ID)) => map!(
                call!(RequestMessage::parse_bytes),
                |request| PeerWireProtocolMessage::Request(request)
            ) |
            (message_len, Some(PIECE_MESSAGE_ID)) => map!(
                call!(PieceMessage::parse_bytes, message_len - 9),
                |piece| PeerWireProtocolMessage::Piece(piece)
            ) |
            (CANCEL_MESSAGE_LEN, Some(CANCEL_MESSAGE_ID)) => map!(
                call!(CancelMessage::parse_bytes),
                |cancel| PeerWireProtocolMessage::Cancel(cancel)
            )
         ) | map!(call!(BitsExtensionMessage::parse_bytes), |bits_ext| PeerWireProtocolMessage::BitsExtension(bits_ext))
           | map!(call!(call_parse_bytes, ext_protocol),    |prot_ext| PeerWireProtocolMessage::ProtExtension(prot_ext)))
}

fn call_parse_bytes<'a, P>(bytes: &'a [u8], protocol: &mut P) -> IResult<&'a [u8], P::ProtocolMessage>
    where P: PeerProtocol {
    protocol.parse_bytes(bytes)
}