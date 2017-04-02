fn main() { }
/*extern crate bip_handshake;
extern crate bip_peer;
extern crate bip_util;
extern crate chan;
extern crate bip_metainfo;

use std::io::{Write, Read};
use std::sync::mpsc::{self, Sender, Receiver};
use std::fs::File;
use std::path::Path;

use bip_metainfo::MetainfoFile;
use bip_handshake::{Handshaker, BTHandshaker};
use bip_peer::disk::{ODiskMessage, IDiskMessage, DiskManagerAccess};
use bip_peer::disk::{DiskManagerRegistration, DiskManager};
use bip_peer::disk::fs::{FileSystem};
use bip_peer::disk::fs::native::{NativeFileSystem};
use bip_peer::protocol::{self, OProtocolMessage, OProtocolMessageKind};
use bip_peer::selector::{OSelectorMessage, OSelectorMessageKind};
use bip_peer::LayerRegistration;
use bip_peer::token::{Token};
use bip_util::send::TrySender;
use bip_peer::message::standard::{RequestMessage, HaveMessage};

// We haven't implemented an actual selection layer yet, so this will suffice
struct MockSelectionRegistration {
    send: SelectionSender
}
impl LayerRegistration<OSelectorMessage, OProtocolMessage> for MockSelectionRegistration {
    type SS2 = SelectionSender;

    fn register(&mut self, _send: Box<TrySender<OSelectorMessage>>) -> SelectionSender {
        self.send.clone()
    }
}

#[derive(Clone)]
struct SelectionSender(Sender<ISelectionMessage>);

enum ISelectionMessage {
    Peer(OProtocolMessage),
    Disk(ODiskMessage)
}

impl From<OProtocolMessage> for ISelectionMessage {
    fn from(data: OProtocolMessage) -> ISelectionMessage {
        ISelectionMessage::Peer(data)
    }
}

impl From<ODiskMessage> for ISelectionMessage {
    fn from(data: ODiskMessage) -> ISelectionMessage {
        ISelectionMessage::Disk(data)
    }
}

impl<T> TrySender<T> for SelectionSender
    where T: Into<ISelectionMessage> + Send {
    fn try_send(&self, data: T) -> Option<T> {
        self.0.send(data.into());

        None
    }
}

// ----------------------------------------------------------------------------//

// Read in a metainfo file and return it.
fn read_to_metainfo<P>(path: P) -> MetainfoFile
    where P: AsRef<Path> {
    let mut buffer = Vec::new();
    let mut file = File::open(path).unwrap();

    file.read_to_end(&mut buffer).unwrap();

    MetainfoFile::from_bytes(buffer).unwrap()
}

// Setup all the parts necessary for returning the components used by a selection layer.
fn setup_selection_layer_parts() -> (BTHandshaker<Sender<()>, ()>, Receiver<ISelectionMessage>, DiskManager) {
    let (metadata_send, _metadata_recv): (Sender<()>, Receiver<()>) = mpsc::channel();
    let (select_send, select_recv) = mpsc::channel();

    // Create our mock selection layer registration
    let selection_registration = MockSelectionRegistration{ send: SelectionSender(select_send) };

    // Create our real native fs disk manager registration
    let fs = NativeFileSystem::with_directory("C:\\Users\\GG\\Downloads\\BIP");
    let mut disk_registration = DiskManagerRegistration::with_fs(fs);

    // Register ourselves with the disk manager so we can add torrents for tracking purposes
    let disk_manager = disk_registration.register(Box::new(selection_registration.send.clone()));
    
    // Create a new handshaker which will also create the protocol layer for us, it will be in charge of registering itself
    // with our selection layer, as well as the disk layer on a per peer connection bases so we can track the peers that connect.
    // We pass in a dummy metadata sender because I am not interested in receiving metadata from our discovery sources.
    let pid = ['-' as u8, 'U' as u8, 'T' as u8, '5' as u8, '5' as u8, '5' as u8, '5' as u8, '-' as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].into();
    let handshaker = protocol::spawn_tcp_handshaker(metadata_send, "0.0.0.0:0".parse().unwrap(), pid, disk_registration, selection_registration).unwrap();

    // Our selection layer will use these three components to orchestrate the downloading of torrents and discovering new peers that
    // connect to us from our protocol layer as well as any "events" that occur in the disk layer, like a good or bad piece detected
    (handshaker, select_recv, disk_manager)
}

fn main() {
    let (mut handshaker, select_recv, disk_manager) = setup_selection_layer_parts();

    disk_manager.try_send(IDiskMessage::AddTorrent(read_to_metainfo("C:\\Users\\GG\\Desktop\\Test.torrent")));
    
    // I guess I forgot to implement clone...
    let metainfo_file = read_to_metainfo("C:\\Users\\GG\\Desktop\\Test.torrent");
    if let ISelectionMessage::Disk(ODiskMessage::TorrentAdded(hash)) = select_recv.recv().unwrap() {
        println!("Torrent Added To Disk Manager");
        assert_eq!(hash, metainfo_file.info_hash());
    } else { panic!("Torrent Added Message Expected!!!") }

    // Torrent has been added, time to open the flood gate...
    handshaker.register(metainfo_file.info_hash());

    // For now just connect to deluge locally...
    handshaker.connect(None, metainfo_file.info_hash(), "10.0.0.18:57229".parse().unwrap());

    // Peer connect message should come next
    let (id, peer_channel) = if let ISelectionMessage::Peer(message) = select_recv.recv().unwrap() {
        let (id, message_kind) = message.destroy();

        if let OProtocolMessageKind::PeerConnect(channel, hash) = message_kind {
            println!("Peer Connected For {:?}: {:?}", hash, id);

            (id, channel)
        } else { panic!("Peer Connect Message Expected!!!") }
    } else { panic!("Peer Message Expected!!!") };

    // Peer bitfield should come next
    if let ISelectionMessage::Peer(message) = select_recv.recv().unwrap() {
        let (id, message_kind) = message.destroy();

        if let OProtocolMessageKind::PeerBitField(_) = message_kind {
            println!("Peer Sent BitFiled");
        } else { panic!("Peer BitField Expected") }
    }

    // Set interest
    assert_eq!(peer_channel.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerInterested)), None);

    // Peer should unchoke us
    if let ISelectionMessage::Peer(message) = select_recv.recv().unwrap() {
        let (id, message_kind) = message.destroy();

        if let OProtocolMessageKind::PeerUnChoke = message_kind {
            println!("Peer Has Unchoked Us");
        } else { panic!("Peer Unchoke Expected") }
    }

    // Deluge wont use higher than 16 KB blocks?
    let max_block_size = 1024 * 16;

    let total_pieces = metainfo_file.info().pieces().count();
    let total_files_size: i64 = metainfo_file.info().files().map(|file| file.length()).sum();
    let piece_length = metainfo_file.info().piece_length();
    let last_piece_size = total_files_size - ((total_pieces - 1) * piece_length as usize) as i64;

    #[derive(Copy, Clone, PartialEq, Eq)]
    enum PieceState {
        Bad,
        Good,
        Requested
    }
    let mut good_pieces = vec![PieceState::Bad; total_pieces];
    let mut request_buffer = Vec::new();

    while !good_pieces.iter().all(|&is_good| is_good == PieceState::Good) {
        println!("1");
        // See if we need/can send out any requests for pieces
        if let Some(position) = good_pieces.iter().position(|&is_good| is_good == PieceState::Bad) {
            println!("2");
            // Request all blocks for the piece
            let mut bytes_left = if position == total_pieces - 1 {
                last_piece_size
            } else { piece_length };

            let mut offset = 0;
            while bytes_left != 0 {
                let block_length = std::cmp::min(bytes_left, max_block_size);
                let request = RequestMessage::new(position as u32, offset as u32, block_length as usize);
                request_buffer.push(request);

                bytes_left -= block_length;
                offset += block_length;
            }

            good_pieces[position] = PieceState::Requested;
        }
        println!("Requests Ready: {}", request_buffer.len());
        request_buffer.pop().and_then(|request| {
            println!("Requested Piece {} At Offset {} Length {}", request.piece_index(), request.block_offset(), request.block_length());
            peer_channel.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerRequest(request)));

            Some(())
        });
println!("4");
        loop {
            println!("5");
            // Expect blocks_expected Number Of Blocks But Always Iterate At Least Once
            match select_recv.recv().unwrap() {
                    ISelectionMessage::Disk(ODiskMessage::FoundGoodPiece(hash, index)) => { 
                        println!("Found Good Piece: {}", index); 

                        peer_channel.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerHave(HaveMessage::new(index))));
                        good_pieces[index as usize] = PieceState::Good;
                    },
                    ISelectionMessage::Disk(ODiskMessage::FoundBadPiece(hash, index)) => {
                        println!("Found Bad Piece: {}", index);

                        good_pieces[index as usize] = PieceState::Bad;
                    },
                    ISelectionMessage::Peer(message) => {
                        if let OProtocolMessageKind::PeerPiece(_, msg) = message.destroy().1 {
                            println!("Received Piece {:?} At Offset {:?} Length {:?}", msg.piece_index(), msg.block_offset(), msg.block_length());

                            request_buffer.pop().and_then(|request| {
                                println!("Requested Piece {} At Offset {} Length {}", request.piece_index(), request.block_offset(), request.block_length());
                                peer_channel.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerRequest(request)));

                                Some(())
                            });
                        } else { panic!("Didnt Receive Peer Piece"); }
                    },
                    _ => panic!("Received Unexpected Disk Message")
            }

            if request_buffer.is_empty() {
                break;
            }
        }
        println!("Good Pieces: {}", good_pieces.iter().filter(|&&is_good| is_good == PieceState::Good).count());
        println!("Requested Pieces: {}", good_pieces.iter().filter(|&&is_good| is_good == PieceState::Requested).count());
    }
/*
    let mut good_pieces_count = 0;
    let total_pieces = metainfo_file.info().pieces().count();
    let piece_length = metainfo_file.info().piece_length();

    let mut piece_index = 0;
    while piece_index < total_pieces - 1 {
        let curr_piece_index = piece_index;
        let mut current_block_offset = 0;

        while current_block_offset != piece_length {
            let request = RequestMessage::new(curr_piece_index as u32, current_block_offset as u32, max_block_size);
            peer_channel.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerRequest(request)));
            
            // Either a PeerPiece, GoodPieceMessage, or BadPieceMessage "should" be seen
            match select_recv.recv().unwrap() {
                ISelectionMessage::Disk(ODiskMessage::FoundGoodPiece(hash, index)) => { 
                    println!("Found Good Piece: {}", index); 
                    peer_channel.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerHave(HaveMessage::new(index)))); 

                    good_pieces_count += 1;
                },
                ISelectionMessage::Disk(ODiskMessage::FoundBadPiece(hash, index)) => {
                    println!("Found Bad Piece: {}", index);

                    piece_index = (index - 1) as usize;
                },
                ISelectionMessage::Peer(message) => {
                    if let OProtocolMessageKind::PeerPiece(_, msg) = message.destroy().1 {
                        println!("Received Piece {:?} At Offset {:?} Length {:?}", msg.piece_index(), msg.block_offset(), msg.block_length());
                    } else { panic!("Didnt Receive Peer Piece"); }
                },
                _ => panic!("Received Unexpected Disk Message")
            }

            current_block_offset += max_block_size as i64;
        }

        piece_index += 1;
    }
    
    // Request last piece
    let total_pieces_size: i64 = metainfo_file.info().files().map(|file| file.length()).sum();
    let mut bytes_left = total_pieces_size - ((total_pieces - 1) * piece_length as usize) as i64;
    
    let mut current_block_offset = 0;
    while bytes_left != 0 {
        let block_size = std::cmp::min(bytes_left, max_block_size as i64);
        bytes_left -= block_size;

        let request = RequestMessage::new((total_pieces - 1) as u32, current_block_offset as u32, block_size as usize);
        peer_channel.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerRequest(request)));
            
        // Either a PeerPiece, GoodPieceMessage, or BadPieceMessage "should" be seen
        match select_recv.recv().unwrap() {
            ISelectionMessage::Disk(ODiskMessage::FoundGoodPiece(hash, index)) => { 
                println!("Found Good Piece: {}", index); 
                peer_channel.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerHave(HaveMessage::new(index)))); 

                good_pieces_count += 1;
            },
            ISelectionMessage::Disk(ODiskMessage::FoundBadPiece(hash, index)) => {
                panic!("Found Bad Piece: {}", index);
            },
            ISelectionMessage::Peer(message) => {
                if let OProtocolMessageKind::PeerPiece(_, msg) = message.destroy().1 {
                    println!("Received Piece {:?} At Offset {:?} Length {:?}", msg.piece_index(), msg.block_offset(), msg.block_length());
                } else { panic!("Didnt Receive Peer Piece"); }
            },
            _ => panic!("Received Unexpected Disk Message")
        }

        current_block_offset += block_size as i64;
    }

    loop {
        match select_recv.recv().unwrap() {
            ISelectionMessage::Disk(ODiskMessage::FoundGoodPiece(hash, index)) => { 
                println!("Found Good Piece: {}", index); 
                peer_channel.try_send(OSelectorMessage::new(id, OSelectorMessageKind::PeerHave(HaveMessage::new(index)))); 

                good_pieces_count += 1;
                if index == 1148 {
                    break;
                }
            },
            ISelectionMessage::Disk(ODiskMessage::FoundBadPiece(hash, index)) => {
                panic!("Found Bad Piece: {}", index);
            },
            ISelectionMessage::Peer(message) => {
                if let OProtocolMessageKind::PeerPiece(_, msg) = message.destroy().1 {
                    println!("Received Piece {:?} At Offset {:?} Length {:?}", msg.piece_index(), msg.block_offset(), msg.block_length());
                } else { panic!("Didnt Receive Peer Piece"); }
            },
            _ => panic!("Received Unexpected Disk Message")
        }
    }

    println!("DONE");*/
}*/