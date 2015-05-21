extern crate redox;

use redox::torrent::{TorrentView};
use redox::torrent::metainfo::{Metainfo};

fn main() {
    let metainfo = Metainfo::from_file("tests/data/ubuntu-14.10-desktop-amd64.iso.torrent").unwrap();
    
    println!("Piece Length: {}", metainfo.piece_info().length());
    /*
    for (index, piece) in metainfo.piece_info().pieces().enumerate() {
        print!("Piece Index {}: ", index);
        
        for &byte in piece {
            print!("{:X}", byte);
        }
        print!("\n");
    }
    */
}