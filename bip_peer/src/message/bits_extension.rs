use std::io::{self, Write};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use bip_bencode::{BencodeRef, BDecodeOpt, BConvert, BencodeMut, BMutAccess};
use bip_util::convert;
use bytes::Bytes;
use byteorder::{WriteBytesExt, BigEndian};
use nom::{IResult, be_u32, be_u8, be_u16, Needed};

use message;
use message::bencode;

const PORT_MESSAGE_LEN:          u32 = 3;
const BASE_EXTENDED_MESSAGE_LEN: u32 = 6;

const PORT_MESSAGE_ID:         u8 = 9;
pub const EXTENDED_MESSAGE_ID: u8 = 20;

const EXTENDED_MESSAGE_HANDSHAKE_ID: u8 = 0;

/// Enumeration of messages for `PeerWireProtocolMessage`, activated via `Extensions` bits.
///
/// Sent after the handshake if the corresponding extension bit is set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BitsExtensionMessage {
    /// Messsage for determining the port a peer's DHT is listening on.
    Port(PortMessage),
    /// Message for sending a peer the map of extensions we support.
    Extended(ExtendedMessage)

}

impl BitsExtensionMessage {
    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<BitsExtensionMessage>> {
        parse_extension(bytes)
    }

    pub fn write_bytes<W>(&self, writer: W) -> io::Result<()>
        where W: Write
    {
        match self {
            &BitsExtensionMessage::Port(msg)         => msg.write_bytes(writer),
            &BitsExtensionMessage::Extended(ref msg) => msg.write_bytes(writer)
        }
    }

    pub fn message_size(&self) -> usize {
        match self {
            &BitsExtensionMessage::Port(_)           => PORT_MESSAGE_LEN as usize,
            &BitsExtensionMessage::Extended(ref msg) => BASE_EXTENDED_MESSAGE_LEN as usize + msg.bencode_size()
        }
    }
}

fn parse_extension(mut bytes: Bytes) -> IResult<(), io::Result<BitsExtensionMessage>> {
    let header_bytes = bytes.clone();

    alt!((),
        ignore_input!(
            switch!(header_bytes.as_ref(), throwaway_input!(tuple!(be_u32, be_u8)),
                (PORT_MESSAGE_LEN, PORT_MESSAGE_ID) => map!(
                    call!(PortMessage::parse_bytes, bytes.split_off(message::HEADER_LEN)),
                    |res_port| res_port.map(|port| BitsExtensionMessage::Port(port))
                )
            )
        ) |
        ignore_input!(
            switch!(header_bytes.as_ref(), throwaway_input!(tuple!(be_u32, be_u8, be_u8)),
                (message_len, EXTENDED_MESSAGE_ID, EXTENDED_MESSAGE_HANDSHAKE_ID) => map!(
                    call!(ExtendedMessage::parse_bytes, bytes.split_off(message::HEADER_LEN + 1), message_len - 2),
                    |res_extended| res_extended.map(|extended| BitsExtensionMessage::Extended(extended))
                )
            )
        )
    )
}

// ----------------------------------------------------------------------------//

/// Message for notifying a peer of our DHT port.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct PortMessage {
    port: u16,
}

impl PortMessage {
    pub fn new(port: u16) -> PortMessage {
        PortMessage { port: port }
    }

    pub fn parse_bytes(_input: (), bytes: Bytes) -> IResult<(), io::Result<PortMessage>> {
        match parse_port(bytes.as_ref()) {
            IResult::Done(_, result)  => IResult::Done((), Ok(result)),
            IResult::Error(err)       => IResult::Error(err),
            IResult::Incomplete(need) => IResult::Incomplete(need)
        }
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        try!(message::write_length_id_pair(&mut writer, PORT_MESSAGE_LEN, Some(PORT_MESSAGE_ID)));

        writer.write_u16::<BigEndian>(self.port)
    }
}

fn parse_port(bytes: &[u8]) -> IResult<&[u8], PortMessage> {
    map!(bytes, be_u16, |port| PortMessage::new(port))
}

// ----------------------------------------------------------------------------//

// Terminology is written as if we were receiving the message. Example: Our ip is
// the ip that the sender sees us as. So if were sending this message, it would be
// the ip we see the client as.

const ROOT_ERROR_KEY: &'static str = "ExtendedMessage";

const LT_METADATA_ID: &'static str = "LT_metadata";
const UT_PEX_ID:      &'static str = "ut_pex";

/// Enumeration of extended types activated via `ExtendedMessage`.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ExtendedType {
    LtMetadata,
    UtPex,
    Custom(String)
}

impl ExtendedType {
    pub fn from_id(id: &str) -> ExtendedType {
        match id {
            LT_METADATA_ID => ExtendedType::LtMetadata,
            UT_PEX_ID      => ExtendedType::UtPex,
            custom         => ExtendedType::Custom(custom.to_string())
        }
    }

    pub fn id(&self) -> &str {
        match self {
            &ExtendedType::LtMetadata     => LT_METADATA_ID,
            &ExtendedType::UtPex          => UT_PEX_ID,
            &ExtendedType::Custom(ref id) => &**id
        }
    }
}

/// Message for notifying peers of extensions we support.
///
/// See `http://www.bittorrent.org/beps/bep_0010.html`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtendedMessage {
    id_map:              HashMap<ExtendedType, u8>,
    client_id:           Option<String>,
    client_tcp_port:     Option<u16>,
    our_ip:              Option<IpAddr>,
    client_ipv6_addr:    Option<Ipv6Addr>,
    client_ipv4_addr:    Option<Ipv4Addr>,
    client_max_requests: Option<i64>,
    metadata_size:       Option<i64>,
    raw_bencode:         Bytes
}

impl ExtendedMessage {
    fn with_raw(id_map: HashMap<ExtendedType, u8>, client_id: Option<String>, client_tcp_port: Option<u16>,
                our_ip: Option<IpAddr>, client_ipv6_addr: Option<Ipv6Addr>, client_ipv4_addr: Option<Ipv4Addr>,
                client_max_requests: Option<i64>, metadata_size: Option<i64>, raw_bencode: Bytes) -> ExtendedMessage {
        ExtendedMessage{ id_map: id_map, client_id: client_id, client_tcp_port: client_tcp_port,
            our_ip: our_ip, client_ipv6_addr: client_ipv6_addr, client_ipv4_addr: client_ipv4_addr,
            client_max_requests: client_max_requests, metadata_size: metadata_size, raw_bencode: raw_bencode }      
    }

    pub fn new(id_map: HashMap<ExtendedType, u8>, client_id: Option<String>, client_tcp_port: Option<u16>,
               our_ip: Option<IpAddr>, client_ipv6_addr: Option<Ipv6Addr>, client_ipv4_addr: Option<Ipv4Addr>,
               client_max_requests: Option<i64>, metadata_size: Option<i64>) -> ExtendedMessage {
        let mut message = ExtendedMessage{ id_map: id_map, client_id: client_id, client_tcp_port: client_tcp_port,
            our_ip: our_ip, client_ipv6_addr: client_ipv6_addr, client_ipv4_addr: client_ipv4_addr,
            client_max_requests: client_max_requests, metadata_size: metadata_size, raw_bencode: Bytes::new() };

        let raw_bencode_bytes = bencode_from_extended_params(&message);
        message.raw_bencode.extend_from_slice(&raw_bencode_bytes);
        
        message
    }

    pub fn from_bencode(raw_bencode: Bytes) -> io::Result<ExtendedMessage> {
        let raw_bencode_len = raw_bencode.len();

        ExtendedMessage::parse_bytes((), raw_bencode, raw_bencode_len as u32).unwrap().1
    }

    pub fn parse_bytes(_input: (), mut bytes: Bytes, len: u32) -> IResult<(), io::Result<ExtendedMessage>> {
        let cast_len = message::u32_to_usize(len);
        
        if bytes.len() >= cast_len {
            let raw_bencode = bytes.split_to(cast_len);
            let clone_raw_bencode = raw_bencode.clone();

            let res_extended_message = BencodeRef::decode(&*raw_bencode, BDecodeOpt::default())
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))
                .and_then(|bencode| {
                    let ben_dict = try!(bencode::CONVERT.convert_dict(&bencode, ROOT_ERROR_KEY));

                    let id_map = bencode::parse_id_map(ben_dict);
                    let client_id = bencode::parse_client_id(ben_dict);
                    let client_tcp_port = bencode::parse_client_tcp_port(ben_dict);
                    let our_ip = bencode::parse_our_ip(ben_dict);
                    let client_ipv6_addr = bencode::parse_client_ipv6_addr(ben_dict);
                    let client_ipv4_addr = bencode::parse_client_ipv4_addr(ben_dict);
                    let client_max_requests = bencode::parse_client_max_requests(ben_dict);
                    let metadata_size = bencode::parse_metadata_size(ben_dict);

                    Ok(ExtendedMessage::with_raw(id_map, client_id, client_tcp_port, our_ip, client_ipv6_addr,
                                                 client_ipv4_addr, client_max_requests, metadata_size, clone_raw_bencode))
                });
                
            IResult::Done((), res_extended_message)
        } else {
            IResult::Incomplete(Needed::Size(cast_len - bytes.len()))
        }
    }

    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        let real_length = 2 + self.bencode_size();
        try!(message::write_length_id_pair(&mut writer, real_length as u32, Some(EXTENDED_MESSAGE_ID)));

        writer.write_all(&[EXTENDED_MESSAGE_HANDSHAKE_ID]);

        writer.write_all(self.raw_bencode.as_ref())
    }

    pub fn bencode_size(&self) -> usize {
        self.raw_bencode.len()
    }

    pub fn query_id(&self, ext_type: &ExtendedType) -> Option<u8> {
        self.id_map.get(ext_type).map(|id| *id)
    }

    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_ref().map(|id| &**id)
    }

    pub fn client_tcp_port(&self) -> Option<u16> {
        self.client_tcp_port
    }

    pub fn our_ip(&self) -> Option<IpAddr> {
        self.our_ip
    }

    pub fn client_ipv6_addr(&self) -> Option<Ipv6Addr> {
        self.client_ipv6_addr
    }

    pub fn client_ipv4_addr(&self) -> Option<Ipv4Addr> {
        self.client_ipv4_addr
    }

    pub fn client_max_requests(&self) -> Option<i64> {
        self.client_max_requests
    }

    pub fn metadata_size(&self) -> Option<i64> {
        self.metadata_size
    }
}

fn bencode_from_extended_params(extended: &ExtendedMessage) -> Vec<u8> {
    let opt_our_ip = extended.our_ip()
        .map(|our_ip| {
            match our_ip {
                IpAddr::V4(ipv4_addr) => convert::ipv4_to_bytes_be(ipv4_addr).to_vec(),
                IpAddr::V6(ipv6_addr) => convert::ipv6_to_bytes_be(ipv6_addr).to_vec()
            }
        });
    let opt_client_ipv6_addr = extended.client_ipv6_addr()
        .map(|client_ipv6_addr| convert::ipv6_to_bytes_be(client_ipv6_addr));
    let opt_client_ipv4_addr = extended.client_ipv4_addr()
        .map(|client_ipv4_addr| convert::ipv4_to_bytes_be(client_ipv4_addr));

    let mut root_map = BencodeMut::new_dict();
    let mut ben_id_map = BencodeMut::new_dict();

    {
        let root_map_access = root_map.dict_mut().unwrap();

        {
            let ben_id_map_access = ben_id_map.dict_mut().unwrap();
            for (ext_id, &value) in extended.id_map.iter() {
                ben_id_map_access.insert(ext_id.id().as_bytes(), ben_int!(value as i64));
            }
        }

        root_map_access.insert(bencode::ID_MAP_KEY, ben_id_map);
        
        extended.client_id()
            .map(|client_id| root_map_access.insert(bencode::CLIENT_ID_KEY, ben_bytes!(client_id)));
        extended.client_tcp_port()
            .map(|tcp_port| root_map_access.insert(bencode::CLIENT_TCP_PORT_KEY, ben_int!(tcp_port as i64)));
        opt_our_ip
            .as_ref()
            .map(|our_ip| root_map_access.insert(bencode::OUR_IP_KEY, ben_bytes!(our_ip)));
        opt_client_ipv6_addr
            .as_ref()
            .map(|client_ipv6_addr| root_map_access.insert(bencode::CLIENT_IPV6_ADDR_KEY, ben_bytes!(client_ipv6_addr)));
        opt_client_ipv4_addr
            .as_ref()
            .map(|client_ipv4_addr| root_map_access.insert(bencode::CLIENT_IPV4_ADDR_KEY, ben_bytes!(client_ipv4_addr)));
        extended.client_max_requests()
            .map(|client_max_requests| root_map_access.insert(bencode::CLIENT_MAX_REQUESTS_KEY, ben_int!(client_max_requests)));
        extended.metadata_size()
            .map(|metadata_size| root_map_access.insert(bencode::METADATA_SIZE_KEY, ben_int!(metadata_size)));
    }
    
    root_map.encode()
}