extern crate bip_util;
extern crate url;
extern crate base32;

use bip_util::bt::InfoHash;
use bip_util::sha::ShaHash;
use std::default::Default;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Topic {
    BitTorrentInfoHash(InfoHash),
}

impl Topic {
    fn parse(s: &str) -> Option<Self> {
        if s.starts_with("urn:btih:") && s.len() == 9 + 40 {
            // BitTorrent Info Hash, hex
            let mut hash = Vec::with_capacity(20);
            for i in 0..20 {
                let j = 9 + 2 * i;
                match u8::from_str_radix(&s[j..j + 2], 16) {
                    Ok(byte) => hash.push(byte),
                    Err(_) => return None,
                }
            }
            match ShaHash::from_hash(&hash[..]) {
                Ok(sha_hash) => Some(Topic::BitTorrentInfoHash(sha_hash)),
                Err(_) => None,
            }
        } else if s.starts_with("urn:btih:") && s.len() == 9 + 32 {
            // BitTorrent Info Hash, base-32
            base32::decode(base32::Alphabet::RFC4648 { padding: true }, &s[9..])
                .and_then(|hash| match ShaHash::from_hash(&hash[..]) {
                    Ok(sha_hash) => Some(Topic::BitTorrentInfoHash(sha_hash)),
                    Err(_) => None,
                })
        } else {
            None
        }
    }
}

/**
 * From <https://en.wikipedia.org/wiki/Magnet_URI_scheme#Parameters>:
 *
 * dn (Display Name) – Filename
 * xl (eXact Length) – Size in bytes
 * xt (eXact Topic) – URN containing file hash
 * as (Acceptable Source) – Web link to the file online
 * xs (eXact Source) – P2P link.
 * kt (Keyword Topic) – Key words for search
 * mt (Manifest Topic) – link to the metafile that contains a list of magneto (MAGMA – MAGnet MAnifest)
 * tr (address TRacker) – Tracker URL for BitTorrent downloads
 **/
#[derive(Clone, Debug)]
pub struct MagnetLink {
    display_name: Option<String>,
    exact_length: Option<usize>,
    exact_topic: Option<Topic>,
    acceptable_source: Vec<String>,
    exact_source: Vec<String>,
    keyword_topic: Vec<String>,
    manifest_topic: Option<String>,
    address_tracker: Vec<String>,
}

impl Default for MagnetLink {
    fn default() -> Self {
        MagnetLink {
            display_name: None,
            exact_length: None,
            exact_topic: None,
            acceptable_source: vec![],
            exact_source: vec![],
            keyword_topic: vec![],
            manifest_topic: None,
            address_tracker: vec![],
        }
    }
}

impl MagnetLink {
    pub fn parse(s: &str) -> Option<Self> {
        // Parse URL
        let url = match Url::parse(s) {
            Ok(url) => url,
            Err(_) => return None,
        };
        // Is Magnet Link?
        if url.scheme != "magnet" {
            return None;
        };

        // Gather Magnet Link data from query string
        let mut result: Self = Default::default();
        let pairs = match url.query_pairs() {
            Some(pairs) => pairs,
            None => return None,
        };
        for (k, v) in pairs {
            match &k[..] {
                "dn" => result.display_name = Some(v),
                "xl" => {
                    match usize::from_str_radix(&v[..], 10) {
                        Ok(exact_length) => result.exact_length = Some(exact_length),
                        Err(_) => (),
                    }
                }
                "xt" => {
                    match Topic::parse(&v[..]) {
                        Some(topic) => result.exact_topic = Some(topic),
                        None => (),
                    }
                }
                "as" => result.acceptable_source.push(v),
                "xs" => result.exact_source.push(v),
                "kt" => result.keyword_topic.push(v),
                "mt" => result.manifest_topic = Some(v),
                "tr" => result.address_tracker.push(v),
                _ => (),
            }
        }

        Some(result)
    }

    pub fn get_info_hash(&self) -> Option<InfoHash> {
        match self.exact_topic {
            Some(Topic::BitTorrentInfoHash(info_hash)) => Some(info_hash),
            _ => None,
        }
    }
}


#[cfg(test)]
mod tests {
    use bip_util::sha::ShaHash;

    #[test]
    fn test_wikipedia() {
        let url = "magnet:?xt=urn:ed2k:354B15E68FB8F36D7CD88FF94116CDC1
&xt=urn:btih:QHQXPYWMACKDWKP47RRVIV7VOURXFE5Q
&xt=urn:tree:tiger:7N5OAMRNGMSSEUE3ORHOKWN4WWIQ5X4EBOOTLJY
&xl=10826029&dn=mediawiki-1.15.1.tar.gz
&tr=udp%3A%2F%2Ftracker.openbittorrent.com%3A80%2Fannounce
&as=http%3A%2F%2Fdownload.wikimedia.org%2Fmediawiki%2F1.15%2Fmediawiki-1.15.1.tar.gz
&xs=http%3A%2F%2Fcache.example.org%2FXRX2PEFXOOEJFRVUCX6HMZMKS5TWG4K5
&xs=dchub://example.org";
        let link = ::MagnetLink::parse(url).unwrap();

        let expected_info_hash = [129, 225, 119, 226, 204, 0, 148, 59, 41, 252, 252, 99, 84, 87,
                                  245, 117, 35, 114, 147, 176];
        assert_eq!(link.get_info_hash(),
                   Some(ShaHash::from_hash(&expected_info_hash[..]).unwrap()));

        assert_eq!(link.exact_length, Some(10826029));
        assert_eq!(link.display_name,
                   Some("mediawiki-1.15.1.tar.gz".to_string()));
        assert_eq!(link.address_tracker,
                   vec!["udp://tracker.openbittorrent.com:80/announce"]);
        assert_eq!(link.acceptable_source,
                   vec!["http://download.wikimedia.org/mediawiki/1.15/mediawiki-1.15.1.tar.gz"]);
        assert_eq!(link.exact_source,
                   vec!["http://cache.example.org/XRX2PEFXOOEJFRVUCX6HMZMKS5TWG4K5",
                        "dchub://example.org"]);
    }

    #[test]
    fn test_tpb() {
        let url = "magnet:?xt=urn:btih:\
                   d9be6909325d28912f400fcb324005dd5861e49f&dn=Crunchbang+GNU%2FLinux+-+AMD64+ISO&tr=udp%3A%2F%2Ftracker.\
                   openbittorrent.com%3A80&tr=udp%3A%2F%2Fopen.demonii.\
                   com%3A1337&tr=udp%3A%2F%2Ftracker.coppersurfer.tk%3A6969&tr=udp%3A%2F%2Fexodus.\
                   desync.com%3A6969";
        let link = ::MagnetLink::parse(url).unwrap();

        let expected_info_hash = [0xd9, 0xbe, 0x69, 0x09, 0x32, 0x5d, 0x28, 0x91, 0x2f, 0x40,
                                  0x0f, 0xcb, 0x32, 0x40, 0x05, 0xdd, 0x58, 0x61, 0xe4, 0x9f];
        assert_eq!(link.get_info_hash(),
                   Some(ShaHash::from_hash(&expected_info_hash[..]).unwrap()));

        assert_eq!(link.display_name,
                   Some("Crunchbang GNU/Linux - AMD64 ISO".to_string()));
        assert_eq!(link.address_tracker,
                   vec![
            "udp://tracker.openbittorrent.com:80",
            "udp://open.demonii.com:1337",
            "udp://tracker.coppersurfer.tk:6969",
            "udp://exodus.desync.com:6969",
        ]);
    }
}
