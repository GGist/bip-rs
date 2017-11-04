
use std::io::Write;
use bytes::Bytes;
use message::bencode;
use std::io;
use bip_bencode::{BDecodeOpt, BencodeRef, BConvert};

const REQUEST_MESSAGE_TYPE_ID: u8 = 0;
const DATA_MESSAGE_TYPE_ID:    u8 = 1;
const REJECT_MESSAGE_TYPE_ID:  u8 = 2;

const ROOT_ERROR_KEY: &'static str = "PeerExtensionProtocolMessage";

/// Enumeration of messages for `PeerExtensionProtocolMessage::UtMetadata`.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum UtMetadataMessage {
    Request(UtMetadataRequestMessage),
    Data(UtMetadataDataMessage),
    Reject(UtMetadataRejectMessage)
}

impl UtMetadataMessage {
    pub fn parse_bytes(mut bytes: Bytes) -> io::Result<UtMetadataMessage> {
        // Our bencode is pretty flat, and we dont want to enforce a full decode, as data
        // messages have the raw data appended outside of the bencode structure...
        let decode_opts = BDecodeOpt::new(2, false, false);

        match BencodeRef::decode(bytes.clone().as_ref(), decode_opts) {
            Ok(bencode) => {
                let bencode_dict = try!(bencode::CONVERT.convert_dict(&bencode, ROOT_ERROR_KEY));
                let msg_type = try!(bencode::parse_message_type(bencode_dict));
                let piece = try!(bencode::parse_piece_index(bencode_dict));

                let bencode_bytes = bytes.split_to(bencode.buffer().len());
                let extra_bytes = bytes;

                match msg_type {
                    REQUEST_MESSAGE_TYPE_ID => Ok(UtMetadataMessage::Request(UtMetadataRequestMessage::with_bytes(piece, bencode_bytes))),
                    REJECT_MESSAGE_TYPE_ID  => Ok(UtMetadataMessage::Reject(UtMetadataRejectMessage::with_bytes(piece, bencode_bytes))),
                    DATA_MESSAGE_TYPE_ID    => {
                        let total_size = try!(bencode::parse_total_size(bencode_dict));

                        Ok(UtMetadataMessage::Data(UtMetadataDataMessage::with_bytes(piece, total_size, extra_bytes, bencode_bytes)))
                    },
                    other => { return Err(io::Error::new(io::ErrorKind::Other, format!("Failed To Recognize Message Type For UtMetadataMessage: {}", msg_type))) }
                }
            },
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, format!("Failed To Parse UtMetadataMessage As Bencode: {}", err)))
        }
    }

    pub fn write_bytes<W>(&self, writer: W) -> io::Result<()>
        where W: Write
    {
        match self {
            &UtMetadataMessage::Request(ref request) => request.write_bytes(writer),
            &UtMetadataMessage::Data(ref data)       => data.write_bytes(writer),
            &UtMetadataMessage::Reject(ref reject)   => reject.write_bytes(writer),
        }
    }

    pub fn message_size(&self) -> usize {
        match self {
            &UtMetadataMessage::Request(ref request) => request.message_size(),
            &UtMetadataMessage::Data(ref data)       => data.message_size(),
            &UtMetadataMessage::Reject(ref reject)   => reject.message_size(),
        }
    }
}

// ----------------------------------------------------------------------------//

/// Message for requesting a piece of metadata from a peer.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct UtMetadataRequestMessage {
    piece:        i64,
    bencode_size: usize
}

impl UtMetadataRequestMessage {
    pub fn new(piece: i64) -> UtMetadataRequestMessage {
        let encoded_bytes_size = (ben_map!{
            bencode::MESSAGE_TYPE_KEY => ben_int!(REQUEST_MESSAGE_TYPE_ID as i64),
            bencode::PIECE_INDEX_KEY  => ben_int!(piece)
        }).encode().len();
        
        UtMetadataRequestMessage{ piece: piece, bencode_size: encoded_bytes_size }
    }

    pub fn with_bytes(piece: i64, bytes: Bytes) -> UtMetadataRequestMessage {
        UtMetadataRequestMessage{ piece: piece, bencode_size: bytes.len() }
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        let encoded_bytes = (ben_map!{
            bencode::MESSAGE_TYPE_KEY => ben_int!(REQUEST_MESSAGE_TYPE_ID as i64),
            bencode::PIECE_INDEX_KEY  => ben_int!(self.piece)
        }).encode();

        writer.write_all(encoded_bytes.as_ref())
    }

    pub fn message_size(&self) -> usize {
        self.bencode_size
    }

    pub fn piece(&self) -> i64 {
        self.piece
    }
}

/// Message for sending a piece of metadata from a peer.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct UtMetadataDataMessage {
    piece:        i64,
    total_size:   i64,
    data:         Bytes,
    bencode_size: usize
}

impl UtMetadataDataMessage {
    pub fn new(piece: i64, total_size: i64, data: Bytes) -> UtMetadataDataMessage {
        let encoded_bytes_len = (ben_map!{
            bencode::MESSAGE_TYPE_KEY => ben_int!(DATA_MESSAGE_TYPE_ID as i64),
            bencode::PIECE_INDEX_KEY  => ben_int!(piece),
            bencode::TOTAL_SIZE_KEY   => ben_int!(total_size)
        }).encode().len();

        UtMetadataDataMessage{ piece: piece, total_size: total_size, data: data, bencode_size: encoded_bytes_len }
    }

    pub fn with_bytes(piece: i64, total_size: i64, data: Bytes, bytes: Bytes) -> UtMetadataDataMessage {
        UtMetadataDataMessage{ piece: piece, total_size: total_size, data: data, bencode_size: bytes.len() }
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        let encoded_bytes = (ben_map!{
            bencode::MESSAGE_TYPE_KEY => ben_int!(DATA_MESSAGE_TYPE_ID as i64),
            bencode::PIECE_INDEX_KEY  => ben_int!(self.piece),
            bencode::TOTAL_SIZE_KEY   => ben_int!(self.total_size)
        }).encode();

        try!(writer.write_all(encoded_bytes.as_ref()));

        writer.write_all(self.data.as_ref())
    }

    pub fn message_size(&self) -> usize {
        self.bencode_size + self.data.len()
    }

    pub fn piece(&self) -> i64 {
        self.piece
    }

    pub fn total_size(&self) -> i64 {
        self.total_size
    }

    pub fn data(&self) -> &Bytes {
        &self.data
    }
}

/// Message for rejecting a request for metadata from a peer.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct UtMetadataRejectMessage {
    piece:        i64,
    bencode_size: usize
}

impl UtMetadataRejectMessage {
    pub fn new(piece: i64) -> UtMetadataRejectMessage {
        let encoded_bytes_size = (ben_map!{
            bencode::MESSAGE_TYPE_KEY => ben_int!(REJECT_MESSAGE_TYPE_ID as i64),
            bencode::PIECE_INDEX_KEY  => ben_int!(piece)
        }).encode().len();

        UtMetadataRejectMessage{ piece: piece, bencode_size: encoded_bytes_size }
    }

    pub fn with_bytes(piece: i64, bytes: Bytes) -> UtMetadataRejectMessage {
        UtMetadataRejectMessage{ piece: piece, bencode_size: bytes.len() }
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        let encoded_bytes = (ben_map!{
            bencode::MESSAGE_TYPE_KEY => ben_int!(REJECT_MESSAGE_TYPE_ID as i64),
            bencode::PIECE_INDEX_KEY  => ben_int!(self.piece)
        }).encode();

        writer.write_all(encoded_bytes.as_ref())
    }

    pub fn message_size(&self) -> usize {
        self.bencode_size
    }

    pub fn piece(&self) -> i64 {
        self.piece
    }
}