extern crate bip_handshake;
extern crate bip_peer;
extern crate bip_util;
extern crate chan;
extern crate bip_metainfo;

use std::io::{Write, Read};
use std::sync::mpsc::{self, Sender, Receiver};
use std::fs::File;
use std::path::Path;

use bip_metainfo::MetainfoFile;
use bip_handshake::{Handshaker};
use bip_peer::disk::{ODiskMessage, IDiskMessage, DiskManagerAccess};
use bip_peer::disk::DiskManagerRegistration;
use bip_peer::disk::fs::{FileSystem};
use bip_peer::disk::fs::native::{NativeFileSystem};
use bip_peer::protocol::{self, OProtocolMessage, OProtocolMessageKind};
use bip_peer::selector::{OSelectorMessage, OSelectorMessageKind};
use bip_peer::LayerRegistration;
use bip_peer::token::{Token};
use bip_util::send::TrySender;

struct MockSelectionRegistration {
    send: Sender<OProtocolMessage>,
}
impl LayerRegistration<OSelectorMessage, OProtocolMessage> for MockSelectionRegistration {
    type SS2 = Sender<OProtocolMessage>;

    fn register(&mut self, _send: Box<TrySender<OSelectorMessage>>) -> Sender<OProtocolMessage> {
        self.send.clone()
    }
}

fn read_to_metainfo<P>(path: P) -> MetainfoFile
    where P: AsRef<Path> {
    let mut buffer = Vec::new();
    let mut file = File::open(path).unwrap();

    file.read_to_end(&mut buffer).unwrap();

    MetainfoFile::from_bytes(buffer).unwrap()
}

fn main() {
    let (metadata_send, _metadata_recv): (Sender<()>, Receiver<()>) = mpsc::channel();
    let (protocol_send, protocol_recv) = mpsc::channel();
    let (disk_send, disk_recv) = mpsc::channel();

    let metainfo_file = read_to_metainfo("C:\\Users\\GG\\Desktop\\Test.torrent");

    let selection_registration = MockSelectionRegistration{ send: protocol_send };
    let fs = NativeFileSystem::with_directory("C:\\Users\\GG\\Downloads");
    let mut disk_registration = DiskManagerRegistration::with_fs(fs);

    let selection_layer_dm = disk_registration.register(Box::new(disk_send));
    selection_layer_dm.try_send(IDiskMessage::AddTorrent(metainfo_file));

    for msg in disk_recv {
        println!("{:?}", msg);
    }
}