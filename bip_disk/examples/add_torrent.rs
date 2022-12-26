extern crate bip_disk;
extern crate bip_metainfo;
extern crate futures;

use std::io::{self, Read, Write, BufRead};
use std::fs::File;

use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage};
use bip_disk::fs::NativeFileSystem;
use bip_metainfo::Metainfo;
use futures::{Stream, Sink, Future};

fn main() {
    println!("Utility For Allocating Disk Space For A Torrent File");
    
    let stdin = io::stdin();
    let mut input_lines = stdin.lock().lines();
    let mut stdout = io::stdout();

    print!("Enter the destination download directory: " );
    stdout.flush().unwrap();
    let download_path = input_lines.next().unwrap().unwrap();
    
    print!("Enter the full path to the torrent file: ");
    stdout.flush().unwrap();
    let torrent_path = input_lines.next().unwrap().unwrap();

    let mut torrent_bytes = Vec::new();
    File::open(torrent_path).unwrap().read_to_end(&mut torrent_bytes).unwrap();
    let metainfo_file = Metainfo::from_bytes(torrent_bytes).unwrap();

    let native_fs = NativeFileSystem::with_directory(download_path);
    let disk_manager = DiskManagerBuilder::new().build(native_fs);

    let (disk_send, disk_recv) = disk_manager.split();

    let total_pieces = metainfo_file.info().pieces().count();
    disk_send.send(IDiskMessage::AddTorrent(metainfo_file)).wait().unwrap();

    print!("\n");

    let mut good_pieces = 0;
    for recv_msg in disk_recv.wait() {
        match recv_msg.unwrap() {
            ODiskMessage::TorrentAdded(hash) => {
                println!("Torrent With Hash {:?} Successfully Added", hash);
                println!("Torrent Has {} Good Pieces Out Of {} Total Pieces", good_pieces, total_pieces);
                break;
            }
            ODiskMessage::FoundGoodPiece(_, _) => { good_pieces += 1},
            unexpected => panic!("Unexpected ODiskMessage {:?}", unexpected)
        }
    }
}