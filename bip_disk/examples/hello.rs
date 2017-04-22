extern crate bip_disk;
extern crate bip_metainfo;
extern crate futures;

use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage};
use bip_disk::fs::NativeFileSystem;
use bip_metainfo::MetainfoFile;
use futures::{Stream, Sink, Future};

fn main() {
    let metainfo_bytes = include_bytes!("C://Users//GG/Desktop//Test.torrent");
    let metainfo_file = MetainfoFile::from_bytes(&metainfo_bytes[..]).unwrap();

    let native_fs = NativeFileSystem::with_directory("C://Users//GG//Downloads");
    let disk_manager = DiskManagerBuilder::new()
        .build(native_fs);

    let (send, recv) = disk_manager.split();

    send.send(IDiskMessage::AddTorrent(metainfo_file)).wait().unwrap();

    for out_msg in recv.wait() {
        match out_msg.unwrap() {
            ODiskMessage::TorrentAdded(hash) => println!("Torrent Added: {:?}", hash),
            ODiskMessage::FoundGoodPiece(hash, index) => println!("Found {} As Good Piece", index),
            ODiskMessage::TorrentError(hash, err) => println!("Got Torrent Error: {:?}", err),
            _ => panic!("ASD")
        }
    }
}