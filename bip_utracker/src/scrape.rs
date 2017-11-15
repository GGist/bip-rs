//! Messaging primitives for scraping.

use std::borrow::Cow;
use std::io::{self, Write};

use bip_util::bt::{self, InfoHash};
use bip_util::convert;
use nom::{IResult, Needed, be_i32};

const SCRAPE_STATS_BYTES: usize = 12;

/// Status for a given InfoHash.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ScrapeStats {
    seeders: i32,
    downloaded: i32,
    leechers: i32,
}

impl ScrapeStats {
    /// Create a new ScrapeStats.
    pub fn new(seeders: i32, downloaded: i32, leechers: i32) -> ScrapeStats {
        ScrapeStats {
            seeders: seeders,
            downloaded: downloaded,
            leechers: leechers,
        }
    }

    /// Construct a ScrapeStats from the given bytes.
    fn from_bytes(bytes: &[u8]) -> IResult<&[u8], ScrapeStats> {
        parse_stats(bytes)
    }

    /// Current number of seeders.
    pub fn num_seeders(&self) -> i32 {
        self.seeders
    }

    /// Number of times it has been downloaded.
    pub fn num_downloads(&self) -> i32 {
        self.downloaded
    }

    /// Current number of leechers.
    pub fn num_leechers(&self) -> i32 {
        self.leechers
    }
}

fn parse_stats(bytes: &[u8]) -> IResult<&[u8], ScrapeStats> {
    do_parse!(bytes,
        seeders:    be_i32 >>
        downloaded: be_i32 >>
        leechers:   be_i32 >>
        (ScrapeStats::new(seeders, downloaded, leechers))
    )
}

// ----------------------------------------------------------------------------//

/// Scrape request sent from the client to the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrapeRequest<'a> {
    hashes: Cow<'a, [u8]>,
}

impl<'a> ScrapeRequest<'a> {
    /// Create a new ScrapeRequest.
    pub fn new() -> ScrapeRequest<'a> {
        ScrapeRequest { hashes: Cow::Owned(Vec::new()) }
    }

    /// Construct a ScrapeRequest from the given bytes.
    pub fn from_bytes(bytes: &'a [u8]) -> IResult<&'a [u8], ScrapeRequest<'a>> {
        parse_request(bytes)
    }

    /// Write the ScrapeRequest to the given writer.
    ///
    /// Ordering of the written InfoHash is identical to that of ScrapeRequest::iter().
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        writer.write_all(&*self.hashes)
    }

    /// Add the InfoHash to the current request.
    pub fn insert(&mut self, hash: InfoHash) {
        let hash_bytes: [u8; bt::INFO_HASH_LEN] = hash.into();

        self.hashes.to_mut().extend_from_slice(&hash_bytes);
    }

    /// Iterator over all of the hashes in the request.
    pub fn iter<'b>(&'b self) -> ScrapeRequestIter<'b> {
        ScrapeRequestIter::new(&*self.hashes)
    }

    /// Create an owned version of ScrapeRequest.
    pub fn to_owned(&self) -> ScrapeRequest<'static> {
        ScrapeRequest { hashes: Cow::Owned((*self.hashes).to_vec()) }
    }
}

fn parse_request<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], ScrapeRequest<'a>> {
    let remainder_bytes = bytes.len() % bt::INFO_HASH_LEN;

    if remainder_bytes != 0 {
        IResult::Incomplete(Needed::Size(bt::INFO_HASH_LEN - remainder_bytes))
    } else {
        let end_of_bytes = &bytes[bytes.len()..bytes.len()];

        IResult::Done(end_of_bytes, ScrapeRequest { hashes: Cow::Borrowed(bytes) })
    }
}

// ----------------------------------------------------------------------------//

/// Scrape response sent from the server to the client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrapeResponse<'a> {
    stats: Cow<'a, [u8]>,
}

impl<'a> ScrapeResponse<'a> {
    /// Create a new ScrapeResponse.
    pub fn new() -> ScrapeResponse<'a> {
        ScrapeResponse { stats: Cow::Owned(Vec::new()) }
    }

    /// Construct a ScrapeResponse from the given bytes.
    pub fn from_bytes(bytes: &'a [u8]) -> IResult<&'a [u8], ScrapeResponse<'a>> {
        parse_response(bytes)
    }

    /// Write the ScrapeResponse to the given writer.
    ///
    /// Ordering of the written stats is identical to that of ScrapeResponse::iter().
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write
    {
        writer.write_all(&*self.stats)
    }

    /// Add the scrape statistics to the current response.
    pub fn insert(&mut self, stats: ScrapeStats) {
        let seeders_bytes = convert::four_bytes_to_array(stats.num_seeders() as u32);
        let downloads_bytes = convert::four_bytes_to_array(stats.num_downloads() as u32);
        let leechers_bytes = convert::four_bytes_to_array(stats.num_leechers() as u32);

        self.stats.to_mut().reserve(SCRAPE_STATS_BYTES);

        self.stats.to_mut().extend_from_slice(&seeders_bytes);
        self.stats.to_mut().extend_from_slice(&downloads_bytes);
        self.stats.to_mut().extend_from_slice(&leechers_bytes);
    }

    /// Iterator over each status for every InfoHash in the request.
    ///
    /// Ordering of the status corresponds to the ordering of the InfoHash in the
    /// initial request.
    pub fn iter<'b>(&'b self) -> ScrapeResponseIter<'b> {
        ScrapeResponseIter::new(&*self.stats)
    }

    /// Create an owned version of ScrapeResponse.
    pub fn to_owned(&self) -> ScrapeResponse<'static> {
        ScrapeResponse { stats: Cow::Owned((*self.stats).to_vec()) }
    }
}

fn parse_response<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], ScrapeResponse<'a>> {
    let remainder_bytes = bytes.len() % SCRAPE_STATS_BYTES;

    if remainder_bytes != 0 {
        IResult::Incomplete(Needed::Size(SCRAPE_STATS_BYTES - remainder_bytes))
    } else {
        let end_of_bytes = &bytes[bytes.len()..bytes.len()];

        IResult::Done(end_of_bytes, ScrapeResponse { stats: Cow::Borrowed(bytes) })
    }
}

// ----------------------------------------------------------------------------//

/// Iterator over a number of InfoHashes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ScrapeRequestIter<'a> {
    hashes: &'a [u8],
    offset: usize,
}

impl<'a> ScrapeRequestIter<'a> {
    fn new(bytes: &'a [u8]) -> ScrapeRequestIter<'a> {
        ScrapeRequestIter {
            hashes: bytes,
            offset: 0,
        }
    }
}

impl<'a> Iterator for ScrapeRequestIter<'a> {
    type Item = InfoHash;

    fn next(&mut self) -> Option<InfoHash> {
        if self.offset == self.hashes.len() {
            None
        } else {
            let (start, end) = (self.offset, self.offset + bt::INFO_HASH_LEN);
            self.offset = end;

            Some(InfoHash::from_hash(&self.hashes[start..end]).unwrap())
        }
    }
}

impl<'a> ExactSizeIterator for ScrapeRequestIter<'a> {
    fn len(&self) -> usize {
        self.hashes.len() / bt::INFO_HASH_LEN
    }
}

// ----------------------------------------------------------------------------//

/// Iterator over a number of ScrapeStats.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ScrapeResponseIter<'a> {
    stats: &'a [u8],
    offset: usize,
}

impl<'a> ScrapeResponseIter<'a> {
    fn new(bytes: &'a [u8]) -> ScrapeResponseIter<'a> {
        ScrapeResponseIter {
            stats: bytes,
            offset: 0,
        }
    }
}

impl<'a> Iterator for ScrapeResponseIter<'a> {
    type Item = ScrapeStats;

    fn next(&mut self) -> Option<ScrapeStats> {
        if self.offset == self.stats.len() {
            None
        } else {
            let (start, end) = (self.offset, self.offset + SCRAPE_STATS_BYTES);
            self.offset = end;

            match ScrapeStats::from_bytes(&self.stats[start..end]) {
                IResult::Done(_, stats) => Some(stats),
                _ => panic!("Bug In ScrapeResponseIter Caused ScrapeStats Parsing To Fail..."),
            }
        }
    }
}

impl<'a> ExactSizeIterator for ScrapeResponseIter<'a> {
    fn len(&self) -> usize {
        self.stats.len() / SCRAPE_STATS_BYTES
    }
}

#[cfg(test)]
mod tests {
    use bip_util::bt;
    use byteorder::{BigEndian, WriteBytesExt};
    use nom::IResult;

    use super::{ScrapeRequest, ScrapeResponse, ScrapeStats};

    #[test]
    fn positive_write_request_empty() {
        let mut received = Vec::new();

        let request = ScrapeRequest::new();
        request.write_bytes(&mut received).unwrap();

        let expected = [];

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_request_single_hash() {
        let mut received = Vec::new();

        let expected = [1u8; bt::INFO_HASH_LEN];

        let mut request = ScrapeRequest::new();
        request.insert(expected.into());
        request.write_bytes(&mut received).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_request_many_hashes() {
        let mut received = Vec::new();

        let hash_one = [1u8; bt::INFO_HASH_LEN];
        let hash_two = [34u8; bt::INFO_HASH_LEN];

        let mut expected = Vec::new();
        expected.extend_from_slice(&hash_one);
        expected.extend_from_slice(&hash_two);

        let mut request = ScrapeRequest::new();
        request.insert(hash_one.into());
        request.insert(hash_two.into());
        request.write_bytes(&mut received).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_response_empty() {
        let mut received = Vec::new();

        let response = ScrapeResponse::new();
        response.write_bytes(&mut received).unwrap();

        let expected = [];

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_response_single_stat() {
        let mut received = Vec::new();

        let stat_one = ScrapeStats::new(1234, 2342, 0);

        let mut response = ScrapeResponse::new();
        response.insert(stat_one);
        response.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i32::<BigEndian>(stat_one.num_seeders()).unwrap();
        expected.write_i32::<BigEndian>(stat_one.num_downloads()).unwrap();
        expected.write_i32::<BigEndian>(stat_one.num_leechers()).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_write_response_many_stats() {
        let mut received = Vec::new();

        let stat_one = ScrapeStats::new(1234, 2342, 0);
        let stat_two = ScrapeStats::new(3333, -23, -2323);

        let mut response = ScrapeResponse::new();
        response.insert(stat_one);
        response.insert(stat_two);
        response.write_bytes(&mut received).unwrap();

        let mut expected = Vec::new();
        expected.write_i32::<BigEndian>(stat_one.num_seeders()).unwrap();
        expected.write_i32::<BigEndian>(stat_one.num_downloads()).unwrap();
        expected.write_i32::<BigEndian>(stat_one.num_leechers()).unwrap();

        expected.write_i32::<BigEndian>(stat_two.num_seeders()).unwrap();
        expected.write_i32::<BigEndian>(stat_two.num_downloads()).unwrap();
        expected.write_i32::<BigEndian>(stat_two.num_leechers()).unwrap();

        assert_eq!(&received[..], &expected[..]);
    }

    #[test]
    fn positive_parse_request_empty() {
        let hash_one = [];

        let received = ScrapeRequest::from_bytes(&hash_one);

        let expected = ScrapeRequest::new();

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_request_single_hash() {
        let hash_one = [1u8; bt::INFO_HASH_LEN];

        let received = ScrapeRequest::from_bytes(&hash_one);

        let mut expected = ScrapeRequest::new();
        expected.insert(hash_one.into());

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_request_multiple_hashes() {
        let hash_one = [1u8; bt::INFO_HASH_LEN];
        let hash_two = [5u8; bt::INFO_HASH_LEN];

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&hash_one);
        bytes.extend_from_slice(&hash_two);

        let received = ScrapeRequest::from_bytes(&bytes);

        let mut expected = ScrapeRequest::new();
        expected.insert(hash_one.into());
        expected.insert(hash_two.into());

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_response_empty() {
        let stats_bytes = [];

        let received = ScrapeResponse::from_bytes(&stats_bytes);

        let expected = ScrapeResponse::new();

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_response_single_stat() {
        let stats_bytes = [0, 0, 0, 255, 0, 0, 1, 0, 0, 0, 2, 0];

        let received = ScrapeResponse::from_bytes(&stats_bytes);

        let mut expected = ScrapeResponse::new();
        expected.insert(ScrapeStats::new(255, 256, 512));

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }

    #[test]
    fn positive_parse_response_many_stats() {
        let stats_bytes = [0, 0, 0, 255, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0,
                           3];

        let received = ScrapeResponse::from_bytes(&stats_bytes);

        let mut expected = ScrapeResponse::new();
        expected.insert(ScrapeStats::new(255, 256, 512));
        expected.insert(ScrapeStats::new(1, 2, 3));

        assert_eq!(received, IResult::Done(&b""[..], expected));
    }
}
