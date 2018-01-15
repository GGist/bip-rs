extern crate bip_metainfo;

use std::fs::File;
use std::io::Read;

use bip_metainfo::Metainfo;


fn main() {
    let mut metainfo_bytes = Vec::new();
    File::open("br.torrent").unwrap().read_to_end(&mut metainfo_bytes).unwrap();

    let metainfo: Metainfo = Metainfo::from_bytes(metainfo_bytes).unwrap();
    let trackers = metainfo.trackers().unwrap();

    for tier in trackers.iter() {
        for url in tier.iter() {
            println!("{}", url);
        }
    }
}