fn main() { }

/*extern crate redox;

use std::fs::File;
use std::io::Write;
use redox::torrent::metainfo::Metainfo;
use redox::torrent::{Torrent, InfoView, FileView};
use redox::torrent::extension::{TorrentExt};
use redox::bencode::{Bencode, Bencoded, Bencodable};

fn main() {
    let torrent = Metainfo::from_file("tests/data/test.torrent").unwrap();
    let mut announce_list = torrent.announce_list().unwrap();
    
    for mut i in announce_list.iter() {
        i.check_tier( |tracker| { println!("{}", tracker); false } );
    }
    println!("{:?}", torrent.is_private_tracker());
}*/