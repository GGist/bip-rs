//! Read a Torrent from a file or some bytes.

use std::borrow::{ToOwned};
use std::error::{Error};
use std::fs::{File};
use std::ffi::{AsOsStr};
use std::io::{Read};
use std::mem;
use std::path::{AsPath};

use bencode::{Bencode};
use error::{TorrentError, TorrentResult};
use error::TorrentErrorKind::{Other};
use super::parse;
use super::{ContactType, Torrent, InfoView};
use util::types::{InfoHash};

/// Specialized contact for values which may involve owned storage.
///
/// See torrent::ContactType<'a> for client facing interface.
pub enum Contact<'a> {
    Tracker(&'a str),
    Trackerless(Vec<(&'a str, u16)>),
    Either(&'a str, Vec<(&'a str, u16)>),
    None
}

/// Provides facilities for reading different parts of a metainfo file.
pub struct Metainfo {
    root:          *const Bencode,
    contact:       Contact<'static>,
    comment:       Option<&'static str>,
    created_by:    Option<&'static str>,
    creation_date: Option<i64>,
    info:          InfoView<'static>,
    info_hash:     InfoHash
}

impl Metainfo {
    /// Reads a metainfo file at the given file path.
    pub fn from_file<T>(path: T) -> TorrentResult<Metainfo>
        where T: AsPath {
        let mut torrent_file = try!(File::open(path).map_err( |e|
            TorrentError{ kind: Other, desc: "Problem Opening File", detail: e.detail() }
        ));
        
        let mut torrent_bytes = Vec::new();
        try!(torrent_file.read_to_end(&mut torrent_bytes).map_err( |e|
            TorrentError{ kind: Other, desc: "Problem Reading File", detail: e.detail() }
        ));
        
        Metainfo::from_bytes(&torrent_bytes[..])
    }
    
    /// Reads a metainfo file from the given bencoded bytes.
    pub fn from_bytes(bytes: &[u8]) -> TorrentResult<Metainfo> {
        let bencode = try!(Bencode::from_bytes(bytes).map_err( |e|
            TorrentError{ kind: Other, desc: "Bytes Passed In Are Not Valid Bencode", detail: Some(e.description().to_owned()) }
        ));
        let bencode: *const Bencode = unsafe{ mem::transmute(Box::new(bencode)) };
        
        unsafe {
            let info_view = try!(InfoView::new(&*bencode));
            let info_hash = unsafe{ try!(parse::generate_info_hash(&*bencode)) };
        
            let announce = parse::slice_announce(&*bencode);
            let nodes = parse::slice_nodes(&*bencode);
            
            let contact = match (announce, nodes) {
                (Ok(a), Ok(b))   => Contact::Either(a, b),
                (Ok(a), Err(_))  => Contact::Tracker(a),
                (Err(_), Ok(b))  => Contact::Trackerless(b),
                (Err(_), Err(_)) => Contact::None
            };
            
            Ok(Metainfo{ root: bencode, 
                contact: contact,
                comment: try!(parse::slice_comment(&*bencode)),
                created_by: try!(parse::slice_created_by(&*bencode)),
                creation_date: try!(parse::slice_creation_date(&*bencode)),
                info: info_view,
                info_hash: info_hash }
            )
        }
    }
}

impl Drop for Metainfo {
    fn drop(&mut self) {
        let _: Box<Bencode> = unsafe{ mem::transmute(self.root) };
    }
}

impl Torrent for Metainfo {
    type BencodeType = Bencode;
    
    fn bencode(&self) -> &<Self as Torrent>::BencodeType {
        unsafe{ &*self.root }
    }

    fn contact_type<'a>(&'a self) -> ContactType<'a> {
        match self.contact {
            Contact::Tracker(a)         => ContactType::Tracker(a),
            Contact::Trackerless(ref b) => ContactType::Trackerless(&b[..]),
            Contact::Either(a, ref b)   => ContactType::Either(a, &b[..]),
            Contact::None               => ContactType::None
        }
    }
    
    fn comment(&self) -> Option<&str> {
        self.comment
    }
    
    fn created_by(&self) -> Option<&str> {
        self.created_by
    }
    
    fn creation_date(&self) -> Option<i64> {
        self.creation_date
    }   

    fn info<'a>(&'a self) -> &InfoView<'a> {
        &self.info
    }
    
    fn info_hash(&self) -> &InfoHash {
        &self.info_hash
    }
}