use bencode::{Bencoded};
use self::announce_list::{TierList};
use super::{parse, Torrent};
use util;

pub mod announce_list;

const ANNOUNCE_LIST: &'static str = "announce-list";
const PRIVATE_KEY:   &'static str = "private";

pub trait TorrentExt<'a>: Torrent {
    /// BEP-0027: Private Torrents
    ///
    /// Specifies whether or not this torrent is "allowed" to gather peers from
    /// sources other than those listed in the "announce" or (with BEP-0012)
    /// "announce-list" fields.
    /// Will return false by default if this field is not present in the torrent.
    fn is_private_tracker(&self) -> bool;
    
    /// BEP-0012: Announce List
    ///
    /// Specifies a list of tiers each containing one or more trackers that can
    /// be used to gather peers from.
    /// When using this in conjunction with BEP-0027 special care must be taken
    /// to ensure that when you are moving to a new tracker, all peers from the
    /// previous tracker are dropped.
    fn announce_list(&'a self) -> Option<TierList<'a>>;
}

impl<'a, 'b: 'a, T, B> TorrentExt<'a> for T
    where T: Torrent<BencodeType=B> + 'a, B: Bencoded<Output=B> + 'b {
    fn is_private_tracker(&self) -> bool {
        let bencode = self.bencode();
        let info_dict = parse::slice_info_dict(bencode);
        
        let entry = match info_dict {
            Ok(n)  => n.lookup(PRIVATE_KEY),
            Err(_) => return false
        };
        
        match entry.map(|n| n.int()) {
            Some(Some(n)) => n == 1,
            _             => false
        }
    }
    
    fn announce_list(&'a self) -> Option<TierList<'a>> {
        let bencode = self.bencode();
        let root_dict = parse::slice_root_dict(bencode);
        
        // If any of the bencoded types don't match up, immediately return None
        let tiers = match root_dict.map( |n| n.lookup(ANNOUNCE_LIST).map( |n| n.list() ) ) {
            Ok(Some(Some(n))) => n,
            _                 => return None
        };
        
        // Iterate over the 2D list and grab each UTF-8 encoded announce url
        let mut tier_list = Vec::with_capacity(tiers.len());
        for i in tiers {
            let announce_urls = match i.list() {
                Some(n) => n,
                Non     => return None
            };
            let mut announce_list = Vec::with_capacity(announce_urls.len());
            
            for i in announce_urls {
                match i.str() {
                    Some(n) => announce_list.push(n),
                    None    => return None
                };
            }
            
            // Randomize the current tier as per the specification
            util::fisher_shuffle(&mut announce_list[..]);
            
            tier_list.push(announce_list);
        }
        
        Some(TierList::new(tier_list))
    }
}