use std::io::{self, Write};

use bip_bencode::{BConvert, BDecodeOpt, BencodeRef};
use byteorder::{BigEndian, WriteBytesExt};
use bytes::Bytes;
use nom::{be_u32, be_u8, ErrorKind, IResult};

use crate::message::bencode;
use crate::message::bits_ext;
use crate::message::{self, ExtendedMessage, ExtendedType, PeerWireProtocolMessage};
use crate::protocol::PeerProtocol;

const EXTENSION_HEADER_LEN: usize = message::HEADER_LEN + 1;

mod ut_metadata;

pub use self::ut_metadata::{
    UtMetadataDataMessage, UtMetadataMessage, UtMetadataRejectMessage, UtMetadataRequestMessage,
};

/// Enumeration of `BEP 10` extension protocol compatible messages.
pub enum PeerExtensionProtocolMessage<P>
where
    P: PeerProtocol,
{
    UtMetadata(UtMetadataMessage),
    //UtPex(UtPexMessage),
    Custom(P::ProtocolMessage),
}

impl<P> PeerExtensionProtocolMessage<P>
where
    P: PeerProtocol,
{
    pub fn bytes_needed(bytes: &[u8]) -> io::Result<Option<usize>> {
        // Follows same length prefix logic as our normal wire protocol...
        PeerWireProtocolMessage::<P>::bytes_needed(bytes)
    }

    pub fn parse_bytes(
        bytes: Bytes,
        extended: &ExtendedMessage,
        custom_prot: &mut P,
    ) -> io::Result<PeerExtensionProtocolMessage<P>> {
        match parse_extensions(bytes, extended, custom_prot) {
            IResult::Done(_, result) => result,
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed To Parse PeerExtensionProtocolMessage",
            )),
        }
    }

    pub fn write_bytes<W>(
        &self,
        mut writer: W,
        extended: &ExtendedMessage,
        custom_prot: &mut P,
    ) -> io::Result<()>
    where
        W: Write,
    {
        match self {
            &PeerExtensionProtocolMessage::UtMetadata(ref msg) => {
                let ext_id = if let Some(ext_id) = extended.query_id(&ExtendedType::UtMetadata) {
                    ext_id
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "Can't Send UtMetadataMessage As We Have No Id Mapping",
                    ));
                };

                let total_len = (2 + msg.message_size()) as u32;

                message::write_length_id_pair(
                    &mut writer,
                    total_len,
                    Some(bits_ext::EXTENDED_MESSAGE_ID),
                )?;
                writer.write_u8(ext_id)?;

                msg.write_bytes(writer)
            }
            &PeerExtensionProtocolMessage::Custom(ref msg) => custom_prot.write_bytes(msg, writer),
        }
    }

    pub fn message_size(&self, custom_prot: &mut P) -> usize {
        match self {
            &PeerExtensionProtocolMessage::UtMetadata(ref msg) => msg.message_size(),
            &PeerExtensionProtocolMessage::Custom(ref msg) => custom_prot.message_size(&msg),
        }
    }
}

fn parse_extensions<P>(
    mut bytes: Bytes,
    extended: &ExtendedMessage,
    custom_prot: &mut P,
) -> IResult<(), io::Result<PeerExtensionProtocolMessage<P>>>
where
    P: PeerProtocol,
{
    let header_bytes = bytes.clone();

    // Attempt to parse a built in message type, otherwise, see if it is an
    // extension type.
    alt!(
        (),
        ignore_input!(
            switch!(header_bytes.as_ref(), throwaway_input!(tuple!(be_u32, be_u8, be_u8)),
                (message_len, bits_ext::EXTENDED_MESSAGE_ID, message_id) =>
                    call!(parse_extensions_with_id, bytes.split_off(EXTENSION_HEADER_LEN).split_to(message_len as usize - 2), extended, message_id)
            )
        ) | map!(value!(custom_prot.parse_bytes(bytes)), |res_cust_ext| {
            res_cust_ext.map(PeerExtensionProtocolMessage::Custom)
        })
    )
}

fn parse_extensions_with_id<P>(
    _input: (),
    bytes: Bytes,
    extended: &ExtendedMessage,
    id: u8,
) -> IResult<(), io::Result<PeerExtensionProtocolMessage<P>>>
where
    P: PeerProtocol,
{
    let lt_metadata_id = extended.query_id(&ExtendedType::UtMetadata);
    //let ut_pex_id = extended.query_id(&ExtendedType::UtPex);

    let result = if lt_metadata_id == Some(id) {
        UtMetadataMessage::parse_bytes(bytes).map(PeerExtensionProtocolMessage::UtMetadata)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Unknown Id For PeerExtensionProtocolMessage: {}", id),
        ))
    };

    IResult::Done((), result)
}
