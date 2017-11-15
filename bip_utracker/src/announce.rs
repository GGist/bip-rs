//! Messaging primitives for announcing.

use std::io::{self, Write};
use std::net::{Ipv4Addr, Ipv6Addr};

use bip_util::bt::{self, InfoHash, PeerId};
use bip_util::convert;
use byteorder::{WriteBytesExt, BigEndian};
use nom::{IResult, be_i32, be_i64, be_u32, be_u16, be_u8};

use contact::CompactPeers;
use option::AnnounceOptions;

const IMPLIED_IPV4_ID: [u8; 4] = [0u8; 4];
const IMPLIED_IPV6_ID: [u8; 16] = [0u8; 16];

const DEFAULT_NUM_WANT: i32 = -1;

const ANNOUNCE_NONE_EVENT: i32 = 0;
const ANNOUNCE_COMPLETED_EVENT: i32 = 1;
const ANNOUNCE_STARTED_EVENT: i32 = 2;
const ANNOUNCE_STOPPED_EVENT: i32 = 3;

/// Announce request sent from the client to the server.
///
/// IPv6 is supported but is [not standard](http://opentracker.blog.h3q.com/2007/12/28/the-ipv6-situation/).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnnounceRequest<'a> {
    info_hash: InfoHash,
    peer_id: PeerId,
    state: ClientState,
    ip: SourceIP,
    key: u32,
    num_want: DesiredPeers,
    port: u16,
    options: AnnounceOptions<'a>,
}

impl<'a> AnnounceRequest<'a> {
    /// Create a new AnnounceRequest.
    pub fn new(hash: InfoHash,
               peer_id: PeerId,
               state: ClientState,
               ip: SourceIP,
               key: u32,
               num_want: DesiredPeers,
               port: u16,
               options: AnnounceOptions<'a>)
               -> AnnounceRequest<'a> {
        AnnounceRequest {
            info_hash: hash,
            peer_id: peer_id,
            state: state,
            ip: ip,
            key: key,
            num_want: num_want,
            port: port,
            options: options,
        }
    }

    /// Construct an IPv4 AnnounceRequest from the given bytes.
    pub fn from_bytes_v4(bytes: &'a [u8]) -> IResult<&'a [u8], AnnounceRequest<'a>> {
        parse_request(bytes, SourceIP::from_bytes_v4)
    }

    /// Construct an IPv6 AnnounceRequest from the given bytes.
    pub fn from_bytes_v6(bytes: &'a [u8]) -> IResult<&'a [u8], AnnounceRequest<'a>> {
        parse_request(bytes, SourceIP::from_bytes_v6)
    }

    /// Write the AnnounceRequest to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        try!(writer.write_all(self.info_hash.as_ref()));
        try!(writer.write_all(self.peer_id.as_ref()));

        try!(self.state.write_bytes(&mut writer));
        try!(self.ip.write_bytes(&mut writer));

        try!(writer.write_u32::<BigEndian>(self.key));

        try!(self.num_want.write_bytes(&mut writer));

        try!(writer.write_u16::<BigEndian>(self.port));

        try!(self.options.write_bytes(&mut writer));

        Ok(())
    }

    /// InfoHash of the current request.
    pub fn info_hash(&self) -> InfoHash {
        self.info_hash
    }

    /// PeerId of the current request.
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// State reported by the client in the given request.
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// Source address to send the response to.
    pub fn source_ip(&self) -> SourceIP {
        self.ip
    }

    /// Unique key randomized by the client that the server can use.
    pub fn key(&self) -> u32 {
        self.key
    }

    /// Number of peers desired by the client.
    pub fn num_want(&self) -> DesiredPeers {
        self.num_want
    }

    /// Port to send the response to.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Set of AnnounceOptions supplied in the request.
    pub fn options(&self) -> &AnnounceOptions<'a> {
        &self.options
    }

    /// Create an owned version of AnnounceRequest.
    pub fn to_owned(&self) -> AnnounceRequest<'static> {
        // Do not call clone and simply switch out the AnnounceOptions as that would
        // unecessarily allocate a HashMap with shallowly cloned Cow objects which
        // is superfulous.
        let owned_options = self.options.to_owned();

        AnnounceRequest {
            info_hash: self.info_hash,
            peer_id: self.peer_id,
            state: self.state,
            ip: self.ip,
            key: self.key,
            num_want: self.num_want,
            port: self.port,
            options: owned_options,
        }
    }
}

/// Parse an AnnounceRequest with the given SourceIP type constructor.
fn parse_request<'a>(bytes: &'a [u8],
                     ip_type: fn(bytes: &[u8]) -> IResult<&[u8], SourceIP>)
                     -> IResult<&'a [u8], AnnounceRequest<'a>> {
    do_parse!(bytes,
        info_hash:  map!(take!(bt::INFO_HASH_LEN), |bytes| InfoHash::from_hash(bytes).unwrap()) >>
        peer_id:    map!(take!(bt::PEER_ID_LEN), |bytes| PeerId::from_hash(bytes).unwrap()) >>
        state:      call!(ClientState::from_bytes) >>
        ip:         call!(ip_type) >>
        key:        be_u32 >>
        num_want:   call!(DesiredPeers::from_bytes) >>
        port:       be_u16 >>
        options:    call!(AnnounceOptions::from_bytes) >>
        (AnnounceRequest::new(info_hash, peer_id, state, ip, key, num_want, port, options))
    )
}

// ----------------------------------------------------------------------------//

/// Announce response sent from the server to the client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnnounceResponse<'a> {
    interval: i32,
    leechers: i32,
    seeders: i32,
    peers: CompactPeers<'a>,
}

impl<'a> AnnounceResponse<'a> {
    /// Create a new AnnounceResponse
    pub fn new(interval: i32,
               leechers: i32,
               seeders: i32,
               peers: CompactPeers<'a>)
               -> AnnounceResponse<'a> {
        AnnounceResponse {
            interval: interval,
            leechers: leechers,
            seeders: seeders,
            peers: peers,
        }
    }

    /// Construct an IPv4 AnnounceResponse from the given bytes.
    pub fn from_bytes_v4(bytes: &'a [u8]) -> IResult<&'a [u8], AnnounceResponse<'a>> {
        parse_respone(bytes, CompactPeers::from_bytes_v4)
    }

    /// Construct an IPv6 AnnounceResponse from the given bytes.
    pub fn from_bytes_v6(bytes: &'a [u8]) -> IResult<&'a [u8], AnnounceResponse<'a>> {
        parse_respone(bytes, CompactPeers::from_bytes_v6)
    }

    /// Write the AnnounceResponse to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        try!(writer.write_i32::<BigEndian>(self.interval));
        try!(writer.write_i32::<BigEndian>(self.leechers));
        try!(writer.write_i32::<BigEndian>(self.seeders));

        try!(self.peers.write_bytes(&mut writer));

        Ok(())
    }

    /// Interval in seconds that clients should wait before re-announcing.
    pub fn interval(&self) -> i32 {
        self.interval
    }

    /// Number of leechers the tracker knows about for the torrent.
    pub fn leechers(&self) -> i32 {
        self.leechers
    }

    /// Number of seeders the tracker knows about for the torrent.
    pub fn seeders(&self) -> i32 {
        self.seeders
    }

    /// Peers the tracker knows about that are sharing the torrent.
    pub fn peers(&self) -> &CompactPeers<'a> {
        &self.peers
    }

    /// Create an owned version of AnnounceResponse.
    pub fn to_owned(&self) -> AnnounceResponse<'static> {
        let owned_peers = self.peers().to_owned();

        AnnounceResponse {
            interval: self.interval,
            leechers: self.leechers,
            seeders: self.seeders,
            peers: owned_peers,
        }
    }
}

/// Parse an AnnounceResponse with the given CompactPeers type constructor.
fn parse_respone<'a>(bytes: &'a [u8],
                     peers_type: fn(bytes: &'a [u8]) -> IResult<&'a [u8], CompactPeers<'a>>)
                     -> IResult<&'a [u8], AnnounceResponse<'a>> {
    do_parse!(bytes,
        interval: be_i32 >>
        leechers: be_i32 >>
        seeders:  be_i32 >>
        peers:    call!(peers_type) >>
        (AnnounceResponse::new(interval, leechers, seeders, peers))
    )
}

// ----------------------------------------------------------------------------//

/// Announce state of a client reported to the server.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct ClientState {
    downloaded: i64,
    left: i64,
    uploaded: i64,
    event: AnnounceEvent,
}

impl ClientState {
    /// Create a new ClientState.
    pub fn new(bytes_downloaded: i64,
               bytes_left: i64,
               bytes_uploaded: i64,
               event: AnnounceEvent)
               -> ClientState {
        ClientState {
            downloaded: bytes_downloaded,
            left: bytes_left,
            uploaded: bytes_uploaded,
            event: event,
        }
    }

    /// Construct the ClientState from the given bytes.
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], ClientState> {
        parse_state(bytes)
    }

    /// Write the ClientState to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        try!(writer.write_i64::<BigEndian>(self.downloaded));
        try!(writer.write_i64::<BigEndian>(self.left));
        try!(writer.write_i64::<BigEndian>(self.uploaded));

        try!(self.event.write_bytes(&mut writer));

        Ok(())
    }

    /// Event reported by the client.
    pub fn event(&self) -> AnnounceEvent {
        self.event
    }

    /// Bytes left to be downloaded.
    pub fn bytes_left(&self) -> i64 {
        self.left
    }

    /// Bytes already uploaded.
    pub fn bytes_uploaded(&self) -> i64 {
        self.uploaded
    }

    /// Bytes already downloaded.
    pub fn bytes_downloaded(&self) -> i64 {
        self.downloaded
    }
}

fn parse_state(bytes: &[u8]) -> IResult<&[u8], ClientState> {
    do_parse!(bytes,
        downloaded: be_i64 >>
        left:       be_i64 >>
        uploaded:   be_i64 >>
        event:      call!(AnnounceEvent::from_bytes) >>
        (ClientState::new(downloaded, left, uploaded, event))
    )
}

// ----------------------------------------------------------------------------//

/// Announce event of a client reported to the server.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AnnounceEvent {
    /// No event is reported.
    None,
    /// Torrent download has completed.
    Completed,
    /// Torrent download has started.
    Started,
    /// Torrent download has stopped.
    Stopped,
}

impl AnnounceEvent {
    /// Construct an AnnounceEvent from the given bytes.
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], AnnounceEvent> {
        parse_event(bytes)
    }

    /// Write the AnnounceEvent to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        try!(writer.write_i32::<BigEndian>(self.as_id()));

        Ok(())
    }

    /// Access the raw id of the current event.
    pub fn as_id(&self) -> i32 {
        match self {
            &AnnounceEvent::None => ANNOUNCE_NONE_EVENT,
            &AnnounceEvent::Completed => ANNOUNCE_COMPLETED_EVENT,
            &AnnounceEvent::Started => ANNOUNCE_STARTED_EVENT,
            &AnnounceEvent::Stopped => ANNOUNCE_STOPPED_EVENT,
        }
    }
}

fn parse_event(bytes: &[u8]) -> IResult<&[u8], AnnounceEvent> {
    switch!(bytes, be_i32,
        ANNOUNCE_NONE_EVENT      => value!(AnnounceEvent::None)      |
        ANNOUNCE_COMPLETED_EVENT => value!(AnnounceEvent::Completed) |
        ANNOUNCE_STARTED_EVENT   => value!(AnnounceEvent::Started)   |
        ANNOUNCE_STOPPED_EVENT   => value!(AnnounceEvent::Stopped)
    )
}

// ----------------------------------------------------------------------------//

/// Client specified IP address to send the response to.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum SourceIP {
    /// Infer the IPv4 address from the sender address.
    ImpliedV4,
    /// Send the response to the given IPv4 address.
    ExplicitV4(Ipv4Addr),
    /// Infer the IPv6 address from the sender address.
    ImpliedV6,
    /// Send the response to the given IPv6 address.
    ExplicitV6(Ipv6Addr),
}

impl SourceIP {
    /// Construct the IPv4 SourceIP from the given bytes.
    pub fn from_bytes_v4(bytes: &[u8]) -> IResult<&[u8], SourceIP> {
        parse_preference_v4(bytes)
    }

    /// Construct the IPv6 SourceIP from the given bytes.
    pub fn from_bytes_v6(bytes: &[u8]) -> IResult<&[u8], SourceIP> {
        parse_preference_v6(bytes)
    }

    /// Write the SourceIP to the given writer.
    pub fn write_bytes<W>(&self, writer: W) -> io::Result<()>
        where W: Write
    {
        match self {
            &SourceIP::ImpliedV4 => self.write_bytes_slice(writer, &IMPLIED_IPV4_ID[..]),
            &SourceIP::ImpliedV6 => self.write_bytes_slice(writer, &IMPLIED_IPV6_ID[..]),
            &SourceIP::ExplicitV4(addr) => {
                self.write_bytes_slice(writer, &convert::ipv4_to_bytes_be(addr)[..])
            }
            &SourceIP::ExplicitV6(addr) => {
                self.write_bytes_slice(writer, &convert::ipv6_to_bytes_be(addr)[..])
            }
        }
    }

    /// Whether or not the source is an IPv6 address.
    pub fn is_ipv6(&self) -> bool {
        match self {
            &SourceIP::ImpliedV6 => true,
            &SourceIP::ExplicitV6(_) => true,
            &SourceIP::ImpliedV4 => false,
            &SourceIP::ExplicitV4(_) => false,
        }
    }

    /// Whether or not the source is an IPv4 address.
    pub fn is_ipv4(&self) -> bool {
        !self.is_ipv6()
    }

    /// Write the given byte slice to the given writer.
    fn write_bytes_slice<W>(&self, mut writer: W, bytes: &[u8]) -> io::Result<()>
        where W: Write
    {
        writer.write_all(bytes)
    }
}

fn parse_preference_v4(bytes: &[u8]) -> IResult<&[u8], SourceIP> {
    alt!(bytes,
        tag!(IMPLIED_IPV4_ID) => { |_| SourceIP::ImpliedV4 } |
        parse_ipv4            => { |ipv4| SourceIP::ExplicitV4(ipv4) }
    )
}

named!(parse_ipv4<&[u8], Ipv4Addr>,
    map!(count_fixed!(u8, be_u8, 4), |b| convert::bytes_be_to_ipv4(b))
);

fn parse_preference_v6(bytes: &[u8]) -> IResult<&[u8], SourceIP> {
    alt!(bytes,
        tag!(IMPLIED_IPV6_ID) => { |_| SourceIP::ImpliedV6 } |
        parse_ipv6            => { |ipv6| SourceIP::ExplicitV6(ipv6) }
    )
}

named!(parse_ipv6<&[u8], Ipv6Addr>,
    map!(count_fixed!(u8, be_u8, 16), |b| convert::bytes_be_to_ipv6(b))
);

// ----------------------------------------------------------------------------//

/// Client desired number of peers to send in the response.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum DesiredPeers {
    /// Send the default number of peers.
    Default,
    /// Send a specific number of peers.
    Specified(i32),
}

impl DesiredPeers {
    /// Construct the DesiredPeers from the given bytes.
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], DesiredPeers> {
        parse_desired(bytes)
    }

    /// Write the DesiredPeers to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        let write_value = match self {
            &DesiredPeers::Default => DEFAULT_NUM_WANT,
            &DesiredPeers::Specified(count) => count,
        };
        try!(writer.write_i32::<BigEndian>(write_value));

        Ok(())
    }
}

fn parse_desired(bytes: &[u8]) -> IResult<&[u8], DesiredPeers> {
    // Tuple trick used to subvert the unused pattern warning (nom tries to catch all)
    switch!(bytes, tuple!(be_i32, value!(true)),
        (DEFAULT_NUM_WANT, true) => value!(DesiredPeers::Default) |
        (specified_peers, true)  => value!(DesiredPeers::Specified(specified_peers))
    )
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::io::Write;

    use bip_util::bt::{InfoHash, PeerId};
    use bip_util::convert;
    use byteorder::{WriteBytesExt, BigEndian};
    use nom::IResult;

    use contact::{CompactPeers, CompactPeersV4, CompactPeersV6};
    use option::AnnounceOptions;
    use super::{SourceIP, AnnounceEvent, ClientState, AnnounceRequest, DesiredPeers,
                AnnounceResponse};

    #[test]
    fn positive_write_request() {
        let mut received = Vec::new();

        let info_hash = [3, 4, 2, 3, 4, 3, 1, 6, 7, 56, 3, 234, 2, 3, 4, 3, 5, 6, 7, 8];
        let peer_id = [4, 2, 123, 23, 34, 5, 56, 2, 3, 4, 45, 6, 7, 8, 5, 6, 4, 56, 34, 42];
        let (downloaded, left, uploaded) = (123908, 12309123, 123123);
        let state = ClientState::new(downloaded, left, uploaded, AnnounceEvent::None);
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let key = 234234;
        let num_want = 34;
        let port = 6969;
        let options = AnnounceOptions::new();
        let request = AnnounceRequest::new(info_hash.into(),
                                           peer_id.into(),
                                           state,
                                           SourceIP::ExplicitV4(ip),
                                           key,
                                           DesiredPeers::Specified(num_want),
                                           port,
                                           options.clone());

        request.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_all(&info_hash).unwrap();
        expected.write_all(&peer_id).unwrap();
        expected.write_i64::<BigEndian>(downloaded).unwrap();
        expected.write_i64::<BigEndian>(left).unwrap();
        expected.write_i64::<BigEndian>(uploaded).unwrap();
        expected.write_i32::<BigEndian>(super::ANNOUNCE_NONE_EVENT).unwrap();
        expected.write_all(&convert::ipv4_to_bytes_be(ip)).unwrap();
        expected.write_u32::<BigEndian>(key).unwrap();
        expected.write_i32::<BigEndian>(num_want).unwrap();
        expected.write_u16::<BigEndian>(port).unwrap();
        options.write_bytes(&mut expected).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_response() {
        let mut received = Vec::new();

        let (interval, leechers, seeders) = (213123, 3423423, 2342343);
        let mut peers = CompactPeersV4::new();
        peers.insert("127.0.0.1:2342".parse().unwrap());
        peers.insert("127.0.0.2:0".parse().unwrap());

        let response =
            AnnounceResponse::new(interval, leechers, seeders, CompactPeers::V4(peers.clone()));

        response.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i32::<BigEndian>(interval).unwrap();
        expected.write_i32::<BigEndian>(leechers).unwrap();
        expected.write_i32::<BigEndian>(seeders).unwrap();
        peers.write_bytes(&mut expected).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_state() {
        let mut received = Vec::new();

        let (downloaded, left, uploaded) = (123908, 12309123, 123123);
        let state = ClientState::new(downloaded, left, uploaded, AnnounceEvent::None);
        state.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i64::<BigEndian>(downloaded).unwrap();
        expected.write_i64::<BigEndian>(left).unwrap();
        expected.write_i64::<BigEndian>(uploaded).unwrap();
        expected.write_i32::<BigEndian>(super::ANNOUNCE_NONE_EVENT).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_none_event() {
        let mut received = Vec::new();

        let none_event = AnnounceEvent::None;
        none_event.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i32::<BigEndian>(super::ANNOUNCE_NONE_EVENT).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_completed_event() {
        let mut received = Vec::new();

        let none_event = AnnounceEvent::Completed;
        none_event.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i32::<BigEndian>(super::ANNOUNCE_COMPLETED_EVENT).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_started_event() {
        let mut received = Vec::new();

        let none_event = AnnounceEvent::Started;
        none_event.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i32::<BigEndian>(super::ANNOUNCE_STARTED_EVENT).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_stopped_event() {
        let mut received = Vec::new();

        let none_event = AnnounceEvent::Stopped;
        none_event.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i32::<BigEndian>(super::ANNOUNCE_STOPPED_EVENT).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_source_ipv4_implied() {
        let mut received = Vec::new();

        let implied_ip = SourceIP::ImpliedV4;
        implied_ip.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_all(&super::IMPLIED_IPV4_ID).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_source_ipv6_implied() {
        let mut received = Vec::new();

        let implied_ip = SourceIP::ImpliedV6;
        implied_ip.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_all(&super::IMPLIED_IPV6_ID).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }


    #[test]
    fn positive_write_source_ipv4_explicit() {
        let mut received = Vec::new();

        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let explicit_ip = SourceIP::ExplicitV4(ip);
        explicit_ip.write_bytes(&mut received).unwrap();

        let expected = convert::ipv4_to_bytes_be(ip);

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_source_ipv6_explicit() {
        let mut received = Vec::new();

        let ip = "ADBB:234A:55BD:FF34:3D3A:FFFF:234A:55BD".parse().unwrap();
        let explicit_ip = SourceIP::ExplicitV6(ip);
        explicit_ip.write_bytes(&mut received).unwrap();

        let expected = convert::ipv6_to_bytes_be(ip);

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_desired_peers_default() {
        let mut received = Vec::new();

        let desired_peers = DesiredPeers::Default;
        desired_peers.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i32::<BigEndian>(super::DEFAULT_NUM_WANT).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_desired_peers_specified() {
        let mut received = Vec::new();

        let desired_peers = DesiredPeers::Specified(500);
        desired_peers.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i32::<BigEndian>(500).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_parse_request_empty_options() {
        let info_hash = [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1];
        let peer_id = [2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3];

        let (downloaded, left, uploaded) = (456789, 283465, 200000);
        let state = ClientState::new(downloaded, left, uploaded, AnnounceEvent::Completed);

        let ip = SourceIP::ImpliedV4;
        let (key, num_want, port) = (255123, -102340, 1515);

        let mut bytes = Vec::new();
        bytes.write_all(&info_hash[..]).unwrap();
        bytes.write_all(&peer_id[..]).unwrap();
        bytes.write_i64::<BigEndian>(downloaded).unwrap();
        bytes.write_i64::<BigEndian>(left).unwrap();
        bytes.write_i64::<BigEndian>(uploaded).unwrap();
        bytes.write_i32::<BigEndian>(super::ANNOUNCE_COMPLETED_EVENT).unwrap();
        bytes.write_all(&super::IMPLIED_IPV4_ID).unwrap();
        bytes.write_u32::<BigEndian>(key).unwrap();
        bytes.write_i32::<BigEndian>(num_want).unwrap();
        bytes.write_u16::<BigEndian>(port).unwrap();

        let received = match AnnounceRequest::from_bytes_v4(&bytes) {
            IResult::Done(_, rec) => rec,
            _ => panic!("AnnounceRequest Parsing Failed..."),
        };

        assert_eq!(received.info_hash(), InfoHash::from(info_hash));
        assert_eq!(received.peer_id(), PeerId::from(peer_id));
        assert_eq!(received.state(), state);
        assert_eq!(received.source_ip(), ip);
        assert_eq!(received.key(), key);
        assert_eq!(received.num_want(), DesiredPeers::Specified(num_want));
        assert_eq!(received.port(), port);
    }

    #[test]
    fn negative_parse_request_missing_key() {
        let info_hash = [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1];
        let peer_id = [2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3];

        let (downloaded, left, uploaded) = (456789, 283465, 200000);

        let (_, num_want, port) = (255123, -102340, 1515);

        let mut bytes = Vec::new();
        bytes.write_all(&info_hash[..]).unwrap();
        bytes.write_all(&peer_id[..]).unwrap();
        bytes.write_i64::<BigEndian>(downloaded).unwrap();
        bytes.write_i64::<BigEndian>(left).unwrap();
        bytes.write_i64::<BigEndian>(uploaded).unwrap();
        bytes.write_i32::<BigEndian>(super::ANNOUNCE_COMPLETED_EVENT).unwrap();
        bytes.write_all(&super::IMPLIED_IPV4_ID).unwrap();
        bytes.write_i32::<BigEndian>(num_want).unwrap();
        bytes.write_u16::<BigEndian>(port).unwrap();

        let received = AnnounceRequest::from_bytes_v4(&bytes);

        assert!(received.is_incomplete());
    }

    #[test]
    fn positive_parse_response_empty_peers() {
        let (interval, leechers, seeders) = (34, 234, 0);

        let mut bytes = Vec::new();
        bytes.write_i32::<BigEndian>(interval).unwrap();
        bytes.write_i32::<BigEndian>(leechers).unwrap();
        bytes.write_i32::<BigEndian>(seeders).unwrap();

        let received_v4 = AnnounceResponse::from_bytes_v4(&bytes);
        let received_v6 = AnnounceResponse::from_bytes_v6(&bytes);

        let expected_v4 = AnnounceResponse::new(interval,
                                                leechers,
                                                seeders,
                                                CompactPeers::V4(CompactPeersV4::new()));
        let expected_v6 = AnnounceResponse::new(interval,
                                                leechers,
                                                seeders,
                                                CompactPeers::V6(CompactPeersV6::new()));

        assert_eq!(received_v4, IResult::Done(&b""[..], expected_v4));
        assert_eq!(received_v6, IResult::Done(&b""[..], expected_v6));
    }

    #[test]
    fn positive_parse_response_many_peers() {
        let (interval, leechers, seeders) = (34, 234, 0);

        let mut peers_v4 = CompactPeersV4::new();
        peers_v4.insert("127.0.0.1:3412".parse().unwrap());
        peers_v4.insert("10.0.0.1:2323".parse().unwrap());

        let mut peers_v6 = CompactPeersV6::new();
        peers_v6.insert("[ADBB:234A:55BD:FF34:3D3A:FFFF:234A:55BD]:3432".parse().unwrap());
        peers_v6.insert("[ADBB:234A::FF34:3D3A:FFFF:234A:55BD]:2222".parse().unwrap());

        let mut bytes = Vec::new();
        bytes.write_i32::<BigEndian>(interval).unwrap();
        bytes.write_i32::<BigEndian>(leechers).unwrap();
        bytes.write_i32::<BigEndian>(seeders).unwrap();

        let mut bytes_v4 = bytes.clone();
        peers_v4.write_bytes(&mut bytes_v4).unwrap();

        let mut bytes_v6 = bytes.clone();
        peers_v6.write_bytes(&mut bytes_v6).unwrap();

        let received_v4 = AnnounceResponse::from_bytes_v4(&bytes_v4);
        let received_v6 = AnnounceResponse::from_bytes_v6(&bytes_v6);

        let expected_v4 =
            AnnounceResponse::new(interval, leechers, seeders, CompactPeers::V4(peers_v4));
        let expected_v6 =
            AnnounceResponse::new(interval, leechers, seeders, CompactPeers::V6(peers_v6));

        assert_eq!(received_v4, IResult::Done(&b""[..], expected_v4));
        assert_eq!(received_v6, IResult::Done(&b""[..], expected_v6));
    }

    #[test]
    fn positive_parse_state() {
        let (downloaded, left, uploaded) = (202340, 52340, 5043);

        let mut bytes = Vec::new();
        bytes.write_i64::<BigEndian>(downloaded).unwrap();
        bytes.write_i64::<BigEndian>(left).unwrap();
        bytes.write_i64::<BigEndian>(uploaded).unwrap();
        bytes.write_i32::<BigEndian>(super::ANNOUNCE_NONE_EVENT).unwrap();

        let received = ClientState::from_bytes(&bytes);
        let expected = ClientState::new(downloaded, left, uploaded, AnnounceEvent::None);

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn negative_parse_incomplete_state() {
        let (downloaded, left, uploaded) = (202340, 52340, 5043);

        let mut bytes = Vec::new();
        bytes.write_i64::<BigEndian>(downloaded).unwrap();
        bytes.write_i64::<BigEndian>(left).unwrap();
        bytes.write_i64::<BigEndian>(uploaded).unwrap();

        let received = ClientState::from_bytes(&bytes);

        assert!(received.is_incomplete());
    }

    #[test]
    fn positive_parse_none_event() {
        let mut bytes = Vec::new();
        bytes.write_i32::<BigEndian>(super::ANNOUNCE_NONE_EVENT).unwrap();

        let received = AnnounceEvent::from_bytes(&bytes);
        let expected = AnnounceEvent::None;

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_completed_event() {
        let mut bytes = Vec::new();
        bytes.write_i32::<BigEndian>(super::ANNOUNCE_COMPLETED_EVENT).unwrap();

        let received = AnnounceEvent::from_bytes(&bytes);
        let expected = AnnounceEvent::Completed;

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_started_event() {
        let mut bytes = Vec::new();
        bytes.write_i32::<BigEndian>(super::ANNOUNCE_STARTED_EVENT).unwrap();

        let received = AnnounceEvent::from_bytes(&bytes);
        let expected = AnnounceEvent::Started;

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn negative_parse_no_event() {
        let bytes = [1, 2, 3, 4];

        let received = AnnounceEvent::from_bytes(&bytes);

        assert!(received.is_err());
    }

    #[test]
    fn positive_parse_stopped_event() {
        let mut bytes = Vec::new();
        bytes.write_i32::<BigEndian>(super::ANNOUNCE_STOPPED_EVENT).unwrap();

        let received = AnnounceEvent::from_bytes(&bytes);
        let expected = AnnounceEvent::Stopped;

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_implied_v4_source() {
        let mut bytes = Vec::new();
        bytes.write_all(&super::IMPLIED_IPV4_ID).unwrap();

        let received = SourceIP::from_bytes_v4(&bytes);
        let expected = SourceIP::ImpliedV4;

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_explicit_v4_source() {
        let bytes = [127, 0, 0, 1];

        let received = SourceIP::from_bytes_v4(&bytes);
        let expected = SourceIP::ExplicitV4(Ipv4Addr::new(127, 0, 0, 1));

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_implied_v6_source() {
        let mut bytes = Vec::new();
        bytes.write_all(&super::IMPLIED_IPV6_ID).unwrap();

        let received = SourceIP::from_bytes_v6(&bytes);
        let expected = SourceIP::ImpliedV6;

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_explicit_v6_source() {
        let ip = "ADBB:234A:55BD:FF34:3D3A:FFFF:234A:55BD".parse().unwrap();
        let bytes = convert::ipv6_to_bytes_be(ip);

        let received = SourceIP::from_bytes_v6(&bytes);
        let expected = SourceIP::ExplicitV6(ip);

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }


    #[test]
    fn negative_parse_incomplete_v4_source() {
        let bytes = [0, 0];

        let received = SourceIP::from_bytes_v4(&bytes);

        assert!(received.is_incomplete());
    }

    #[test]
    fn negative_parse_incomplete_v6_source() {
        let bytes = [0, 0, 0, 0];

        let received = SourceIP::from_bytes_v6(&bytes);

        assert!(received.is_incomplete());
    }

    #[test]
    fn negative_parse_empty_v4_source() {
        let bytes = [];

        let received = SourceIP::from_bytes_v4(&bytes);

        assert!(received.is_incomplete());
    }

    #[test]
    fn negative_parse_empty_v6_source() {
        let bytes = [];

        let received = SourceIP::from_bytes_v6(&bytes);

        assert!(received.is_incomplete());
    }

    #[test]
    fn positive_parse_desired_peers_default() {
        let default_bytes = convert::four_bytes_to_array(-1i32 as u32);

        let received = DesiredPeers::from_bytes(&default_bytes);
        let expected = DesiredPeers::Default;

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_desired_peers_specified() {
        let specified_bytes = convert::four_bytes_to_array(50);

        let received = DesiredPeers::from_bytes(&specified_bytes);
        let expected = DesiredPeers::Specified(50);

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }
}
