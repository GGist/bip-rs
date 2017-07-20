extern crate bip_disk;
extern crate bip_handshake;
extern crate bip_metainfo;
extern crate bip_peer;
#[macro_use]
extern crate clap;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_timer;

use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::fs::File;
use std::io::Read;
use std::cmp;

use bip_disk::{DiskManagerBuilder, BlockMetadata, Block, BlockMut, IDiskMessage, ODiskMessage};
use bip_disk::fs::NativeFileSystem;
use bip_disk::fs_cache::FileHandleCache;
//use bip_dht::{DhtBuilder, Handshaker, Router};
use bip_handshake::{HandshakerBuilder, PeerId, InitiateMessage, Protocol, HandshakerConfig};
use bip_handshake::transports::TcpTransport;
use bip_peer::{PeerManagerBuilder, IPeerManagerMessage, PeerInfo, PeerProtocolCodec, OPeerManagerMessage};
use bip_peer::protocols::{PeerWireProtocol, NullProtocol};
use bip_peer::message::{HaveMessage, BitFieldMessage, PeerWireProtocolMessage, PieceMessage, RequestMessage};
use bip_metainfo::{MetainfoFile, InfoDictionary};
use tokio_core::reactor::Core;
use tokio_io::{AsyncRead,};
use futures::{future, stream, Future, Stream, Sink};
use futures::sync::mpsc;
use futures::future::{Loop, Either};

/*
    Things this example doesnt do, because of the lack of bip_select:
      * Logic for piece selection is not abstracted (and is pretty bad)
      * We will unconditionally upload pieces to a peer (regardless whether or not they were choked)
      * We dont add an info hash filter to bip_handshake after we have as many peers as we need/want
      * We dont do any banning of malicious peers
      
    Things the example doesnt do, unrelated to bip_select:
      * Matching peers up to disk requests isnt as good as it could be
      * Doesnt use a shared BytesMut for servicing piece requests
      * Good logging
*/

/*
// Legacy Handshaker, when bip_dht is migrated, it will accept S directly
struct LegacyHandshaker<S> {
    port:   u16,
    id:     PeerId,
    sender: Wait<S>
}

impl<S> LegacyHandshaker<S> where S: DiscoveryInfo + Sink {
    pub fn new(sink: S) -> LegacyHandshaker<S> {
        LegacyHandshaker{ port: sink.port(), id: sink.peer_id(), sender: sink.wait() }
    }
}

impl<S> Handshaker for LegacyHandshaker<S> where S: Sink<SinkItem=InitiateMessage> + Send, S::SinkError: Debug {
    type MetadataEnvelope = ();

    fn id(&self) -> PeerId { self.id }

    fn port(&self) -> u16 { self.port }

    fn connect(&mut self, _expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr) {
        self.sender.send(InitiateMessage::new(Protocol::BitTorrent, hash, addr));
    }

    fn metadata(&mut self, _data: ()) { () }
}
*/

// How many requests can be in flight at once.
const MAX_PENDING_BLOCKS: usize = 50;

// Some enum to store our selection state updates
#[derive(Debug)]
enum SelectState {
    Choke(PeerInfo),
    UnChoke(PeerInfo),
    Interested(PeerInfo),
    UnInterested(PeerInfo),
    Have(PeerInfo, HaveMessage),
    BitField(PeerInfo, BitFieldMessage),
    NewPeer(PeerInfo),
    RemovedPeer(PeerInfo),
    BlockProcessed,
    GoodPiece(u64),
    BadPiece(u64),
    TorrentSynced,
    TorrentAdded
}

fn main() {
    // Command line argument parsing
    let matches = clap_app!(myapp =>
        (version: "1.0")
        (author: "Andrew <amiller4421@gmail.com>")
        (about: "Simple torrent downloading")
        (@arg file: -f +required +takes_value "Location of the torrent file")
        (@arg dir: -d +takes_value "Download directory to use")
        (@arg peer: -p +takes_value "Single peer to connect to of the form addr:port")
    ).get_matches();
    let file = matches.value_of("file").unwrap();
    let dir = matches.value_of("dir").unwrap();
    let peer_addr = matches.value_of("peer").unwrap().parse().unwrap();

    // Load in our torrent file
    let mut metainfo_bytes = Vec::new();
    File::open(file).unwrap().read_to_end(&mut metainfo_bytes).unwrap();

    // Parse out our torrent file
    let metainfo = MetainfoFile::from_bytes(metainfo_bytes).unwrap();
    let info_hash = metainfo.info_hash();

    // Create our main "core" event loop
    let mut core = Core::new().unwrap();

    // Create a disk manager to handle storing/loading blocks (we add in a file handle cache
    // to avoid anti virus causing slow file opens/closes, will cache up to 100 file handles)
    let (disk_manager_send, disk_manager_recv) = DiskManagerBuilder::new()
        .build(FileHandleCache::new(NativeFileSystem::with_directory(dir), 100))
        .into_parts();

    // Create a handshaker that can initiate connections with peers
    let (handshaker_send, handshaker_recv) = HandshakerBuilder::new()
        .with_peer_id(PeerId::from_hash("-BI0000-000000000000".as_bytes()).unwrap())
        // We would ideally add a filter to the handshaker to block
        // peers when we have enough of them for a given hash, but
        // since this is a minimal example, we will rely on peer
        // manager backpressure (handshaker -> peer manager will
        // block when we reach our max peers). Setting these to low
        // values so we dont have more than 2 unused tcp connections.
        .with_config(HandshakerConfig::default()
            .with_wait_buffer_size(0)
            .with_done_buffer_size(0))
        .build::<TcpTransport>(core.handle()) // Will handshake over TCP (could swap this for UTP in the future)
        .unwrap()
        .into_parts();
    // Create a peer manager that will hold our peers and heartbeat/send messages to them
    let (peer_manager_send, peer_manager_recv) = PeerManagerBuilder::new()
        .build(core.handle())
        .into_parts();

    // Hook up a future that feeds incoming (handshaken) peers over to the peer manager
    let map_peer_manager_send = peer_manager_send.clone().sink_map_err(|_| ());
    core.handle().spawn(handshaker_recv
        .map_err(|_| ())
        .map(|complete_msg| {
            // Our handshaker finished handshaking some peer, get
            // the peer info as well as the peer itself (socket)
            let (_, _, hash, pid, addr, sock) = complete_msg.into_parts();
            // Frame our socket with the peer wire protocol with no extensions (nested null protocol), and a max payload of 24KB
            let peer = sock.framed(PeerProtocolCodec::with_max_payload(PeerWireProtocol::new(NullProtocol::new()), 24 * 1024));
            
            // Create our peer identifier used by our peer manager
            let peer_info = PeerInfo::new(addr, pid, hash);

            // Map to a message that can be fed to our peer manager
            IPeerManagerMessage::AddPeer(peer_info, peer)
        })
        .forward(map_peer_manager_send)
        .map(|_| ())
    );

    // Will hold a mapping of BlockMetadata -> Vec<PeerInfo> to track which peers to send a queued block to
    let disk_request_map = Rc::new(RefCell::new(HashMap::new()));
    let (select_send, select_recv) = mpsc::channel(50);

    // Map out the errors for these sinks so they match
    let map_select_send = select_send.clone().sink_map_err(|_| ());
    let map_disk_manager_send = disk_manager_send.clone().sink_map_err(|_| ());

    // Hook up a future that receives messages from the peer manager, and forwards request to the disk manager or selection manager (using loop fn
    // here because we need to be able to access state, like request_map and a different future combinator wouldnt let us keep it around to access)
    core.handle().spawn(future::loop_fn((peer_manager_recv, info_hash, disk_request_map.clone(), map_select_send, map_disk_manager_send), |(peer_manager_recv, info_hash, disk_request_map, select_send, disk_manager_send)| {
        peer_manager_recv.into_future()
            .map_err(|_| ())
            .and_then(move |(opt_item, peer_manager_recv)| {
                let opt_message = match opt_item.unwrap() {
                    OPeerManagerMessage::ReceivedMessage(info, message) => {
                        match message {
                            PeerWireProtocolMessage::Choke              => Some(Either::A(SelectState::Choke(info))),
                            PeerWireProtocolMessage::UnChoke            => Some(Either::A(SelectState::UnChoke(info))),
                            PeerWireProtocolMessage::Interested         => Some(Either::A(SelectState::Interested(info))),
                            PeerWireProtocolMessage::UnInterested       => Some(Either::A(SelectState::UnInterested(info))),
                            PeerWireProtocolMessage::Have(have)         => Some(Either::A(SelectState::Have(info, have))),
                            PeerWireProtocolMessage::BitField(bitfield) => Some(Either::A(SelectState::BitField(info, bitfield))),
                            PeerWireProtocolMessage::Request(request)   => {
                                let block_metadata = BlockMetadata::new(info_hash, request.piece_index() as u64, request.block_offset() as u64, request.block_length());
                                let mut request_map_mut = disk_request_map.borrow_mut();

                                // Add the block metadata to our request map, and add the peer as an entry there
                                let block_entry = request_map_mut.entry(block_metadata);
                                let peers_requested = block_entry.or_insert(Vec::new());

                                peers_requested.push(info);

                                Some(Either::B(IDiskMessage::LoadBlock(BlockMut::new(block_metadata, vec![0u8; block_metadata.block_length()].into()))))
                            },
                            PeerWireProtocolMessage::Piece(piece)       => {
                                let block_metadata = BlockMetadata::new(info_hash, piece.piece_index() as u64, piece.block_offset() as u64, piece.block_length());

                                // Peer sent us a block, send it over to the disk manager to be processed
                                Some(Either::B(IDiskMessage::ProcessBlock(Block::new(block_metadata, piece.block()))))
                            },
                            _                                           => None
                        }
                    },
                    OPeerManagerMessage::PeerAdded(info)        => Some(Either::A(SelectState::NewPeer(info))),
                    OPeerManagerMessage::SentMessage(_, _)      => None,
                    OPeerManagerMessage::PeerRemoved(info)      => { println!("We Removed Peer {:?} From The Peer Manager", info); Some(Either::A(SelectState::RemovedPeer(info))) },
                    OPeerManagerMessage::PeerDisconnect(info)   => { println!("Peer {:?} Disconnected From Us", info); Some(Either::A(SelectState::RemovedPeer(info))) },
                    OPeerManagerMessage::PeerError(info, error) => { println!("Peer {:?} Disconnected With Error: {:?}", info, error); Some(Either::A(SelectState::RemovedPeer(info))) }
                };

                // Could optimize out the box, but for the example, this is cleaner and shorter
                let result_future: Box<Future<Item=Loop<(), _>, Error=()>> = match opt_message {
                    Some(Either::A(select_message)) =>
                        Box::new(select_send.send(select_message).map(move |select_send| Loop::Continue((peer_manager_recv, info_hash, disk_request_map, select_send, disk_manager_send)))),
                    Some(Either::B(disk_message))   =>
                        Box::new(disk_manager_send.send(disk_message).map(move |disk_manager_send| Loop::Continue((peer_manager_recv, info_hash, disk_request_map, select_send, disk_manager_send)))),
                    None                            =>
                        Box::new(future::ok(Loop::Continue((peer_manager_recv, info_hash, disk_request_map, select_send, disk_manager_send))))
                };

                result_future
            })
    }));

    // Map out the errors for these sinks so they match
    let map_select_send = select_send.clone().sink_map_err(|_| ());
    let map_peer_manager_send = peer_manager_send.clone().sink_map_err(|_| ());

    // Hook up a future that receives from the disk manager, and forwards to the peer manager or select manager
    core.handle().spawn(future::loop_fn((disk_manager_recv, disk_request_map.clone(), map_select_send, map_peer_manager_send), |(disk_manager_recv, disk_request_map, select_send, peer_manager_send)| {
        disk_manager_recv.into_future()
            .map_err(|_| ())
            .and_then(|(opt_item, disk_manager_recv)| {
                let opt_message = match opt_item.unwrap() {
                    ODiskMessage::BlockLoaded(block)       => {
                        let (metadata, block) = block.into_parts();

                        // Lookup the peer info given the block metadata
                        let mut request_map_mut = disk_request_map.borrow_mut();
                        let mut peer_list = request_map_mut.get_mut(&metadata).unwrap();
                        let peer_info = peer_list.remove(1);
                        
                        // Pack up our block into a peer wire protocol message and send it off to the peer
                        let piece = PieceMessage::new(metadata.piece_index() as u32, metadata.block_offset() as u32, block.freeze());
                        let pwp_message = PeerWireProtocolMessage::Piece(piece);

                        Some(Either::B(IPeerManagerMessage::SendMessage(peer_info, 0, pwp_message)))
                    },
                    ODiskMessage::TorrentAdded(_)          => Some(Either::A(SelectState::TorrentAdded)),
                    ODiskMessage::TorrentSynced(_)         => Some(Either::A(SelectState::TorrentSynced)),
                    ODiskMessage::FoundGoodPiece(_, index) => Some(Either::A(SelectState::GoodPiece(index))),
                    ODiskMessage::FoundBadPiece(_, index)  => Some(Either::A(SelectState::BadPiece(index))),
                    ODiskMessage::BlockProcessed(_)        => Some(Either::A(SelectState::BlockProcessed)),
                    _                                      => None
                };

                // Could optimize out the box, but for the example, this is cleaner and shorter
                let result_future: Box<Future<Item=Loop<(), _>, Error=()>> = match opt_message {
                    Some(Either::A(select_message)) =>
                        Box::new(select_send.send(select_message).map(|select_send| Loop::Continue((disk_manager_recv, disk_request_map, select_send, peer_manager_send)))),
                    Some(Either::B(peer_message))   =>
                        Box::new(peer_manager_send.send(peer_message).map(|peer_manager_send| Loop::Continue((disk_manager_recv, disk_request_map, select_send, peer_manager_send)))),
                    None                            =>
                        Box::new(future::ok(Loop::Continue((disk_manager_recv, disk_request_map, select_send, peer_manager_send))))
                };

                result_future
            })
        })
    );

    // Generate data structure to track the requests we need to make, the requests that have been fulfilled, and an active peers list
    let piece_requests = generate_requests(metainfo.info(), 16 * 1024);

    // Have our disk manager allocate space for our torrent and start tracking it
    core.run(disk_manager_send.send(IDiskMessage::AddTorrent(metainfo.clone()))).unwrap();

    // For any pieces we already have on the file system (and are marked as good), we will be removing them from our requests map
    let (select_recv, piece_requests, cur_pieces) = core.run(future::loop_fn((select_recv, piece_requests, 0), |(select_recv, mut piece_requests, cur_pieces)| {
        select_recv.into_future()
            .map(move |(opt_item, select_recv)| {
                match opt_item.unwrap() {
                    // Disk manager identified a good piece already downloaded
                    SelectState::GoodPiece(index) => {
                        piece_requests = piece_requests.into_iter()
                            .filter(|req| req.piece_index() != index as u32)
                            .collect();
                        Loop::Continue((select_recv, piece_requests, cur_pieces + 1))
                    },
                    // Disk manager is finished identifying good pieces, torrent has been added
                    SelectState::TorrentAdded     => Loop::Break((select_recv, piece_requests, cur_pieces)),
                    // Shouldnt be receiving any other messages...
                    message                       => panic!("Unexpected Message Received In Selection Receiver: {:?}", message)
                }
            })
            .map_err(|_| ())
    })).unwrap();

/*
    // Setup the dht which will be the only peer discovery service we use in this example
    let legacy_handshaker = LegacyHandshaker::new(handshaker_send);
    let dht = DhtBuilder::with_router(Router::uTorrent)
        .set_read_only(false)
        .start_mainline(legacy_handshaker).unwrap();

    dht.search(info_hash, true);
*/

    // Send the peer given from the command line over to the handshaker to initiate a connection
    core.run(handshaker_send.send(InitiateMessage::new(Protocol::BitTorrent, info_hash, peer_addr)).map_err(|_| ())).unwrap();

    // Finally, setup our main event loop to drive the tasks we setup earlier
    let map_peer_manager_send = peer_manager_send.sink_map_err(|_| ());
    let total_pieces = metainfo.info().pieces().count();
    println!("Current Pieces: {}\nTotal Pieces: {}\nRequests Left: {}", cur_pieces, total_pieces, piece_requests.len());

    let result: Result<(), ()> = core.run(future::loop_fn((select_recv, map_peer_manager_send, piece_requests, None, false, 0, cur_pieces, total_pieces),
        |(select_recv, map_peer_manager_send, mut piece_requests, mut opt_peer, mut unchoked, mut blocks_pending, mut cur_pieces, total_pieces)| {
            select_recv.into_future()
                .map_err(|_| ())
                .and_then(move |(opt_message, select_recv)| {
                    // Handle the current selection messsage, decide any control messages we need to send
                    let send_messages = match opt_message.unwrap() {
                        SelectState::BlockProcessed    => {
                            // Disk manager let us know a block was processed (one of our requests made it
                            // from the peer manager, to the disk manager, and this is the acknowledgement)
                            blocks_pending -= 1;
                            vec![]
                        },
                        SelectState::Choke(_)          => {
                            // Peer choked us, cant be sending any requests to them for now
                            unchoked = false;
                            vec![]
                        },
                        SelectState::UnChoke(_)        => {
                            // Peer unchoked us, we can continue sending sending requests to them
                            unchoked = true;
                            vec![] },
                        SelectState::NewPeer(info)     => {
                            // A new peer connected to us, store its contact info (just supported one peer atm),
                            // and go ahead and express our interest in them, and unchoke them (we can upload to them)
                            // We dont send a bitfield message (just to keep things simple).
                            opt_peer = Some(info);
                            vec![IPeerManagerMessage::SendMessage(info, 0, PeerWireProtocolMessage::Interested),
                                 IPeerManagerMessage::SendMessage(info, 0, PeerWireProtocolMessage::UnChoke)]
                        },
                        SelectState::GoodPiece(piece)  => {
                            // Disk manager has processed endough blocks to make up a piece, and that piece
                            // was verified to be good (checksummed). Go ahead and increment the number of
                            // pieces we have. We dont handle bad pieces here (since we deleted our request
                            // but ideally, we would recreate those requests and resend/blacklist the peer).
                            cur_pieces += 1;
                            
                            if let Some(peer) = opt_peer {
                                // Send our have message back to the peer
                                vec![IPeerManagerMessage::SendMessage(peer, 0, PeerWireProtocolMessage::Have(HaveMessage::new(piece as u32)))]
                            }
                            else {
                                vec![]
                            }
                        },
                        // Decided not to handle these two cases here
                        SelectState::RemovedPeer(info) => panic!("Peer {:?} Got Disconnected", info),
                        SelectState::BadPiece(_)       => panic!("Peer Gave Us Bad Piece"),
                        _                              => vec![]
                    };

                    // Need a type annotation of this return type, provide that
                    let result: Box<Future<Item=Loop<_, _>, Error=()>> = if cur_pieces == total_pieces {
                        // We have all of the (unique) pieces required for our torrent
                        Box::new(future::ok(Loop::Break(())))
                    } else if let Some(peer) = opt_peer {
                        // We have peer contact info, if we are unchoked, see if we can queue up more requests
                        let next_piece_requests = if unchoked {
                            let take_blocks = cmp::min(MAX_PENDING_BLOCKS - blocks_pending, piece_requests.len());
                            blocks_pending += take_blocks;

                            piece_requests.drain(0..take_blocks)
                                .map(move |item| Ok::<_, ()>(IPeerManagerMessage::SendMessage(peer, 0, PeerWireProtocolMessage::Request(item))))
                                .collect()
                        } else {
                            vec![]
                        };

                        // First, send any control messages, then, send any more piece requests
                        Box::new(map_peer_manager_send
                            .send_all(stream::iter(send_messages.into_iter().map(Ok::<_, ()>)))
                            .map_err(|_| ())
                            .and_then(|(map_peer_manager_send, _)| {
                                map_peer_manager_send.send_all(stream::iter(next_piece_requests))
                            })
                            .map_err(|_| ())
                            .map(move |(map_peer_manager_send, _)| {
                                Loop::Continue((select_recv, map_peer_manager_send, piece_requests, opt_peer, unchoked, blocks_pending, cur_pieces, total_pieces))
                            })
                        )
                    } else {
                        // Not done yet, and we dont have any peer info stored (havent received the peer yet)
                        Box::new(future::ok(Loop::Continue((select_recv, map_peer_manager_send, piece_requests, opt_peer, unchoked, blocks_pending, cur_pieces, total_pieces))))
                    };

                    result
                })
    }));

    result.unwrap();
}

/// Generate a mapping of piece index to list of block requests for that piece, given a block size.
///
/// Note, most clients will drop connections for peers requesting block sizes above 16KB.
fn generate_requests(info: &InfoDictionary, block_size: usize) -> Vec<RequestMessage> {
    let mut requests = Vec::new();
    
    // Grab our piece length, and the sum of the lengths of each file in the torrent
    let piece_len: u64 = info.piece_length();
    let mut total_file_length: u64 = info.files().map(|file| file.length()).sum();

    // Loop over each piece (keep subtracting total file length by piece size, use cmp::min to handle last, smaller piece)
    let mut piece_index: u64 = 0;
    while total_file_length != 0 {
        let next_piece_len = cmp::min(total_file_length, piece_len);

        // For all whole blocks, push the block index and block_size
        let whole_blocks = next_piece_len / block_size as u64;
        for block_index in 0..whole_blocks {
            let block_offset = block_index * block_size as u64;

            requests.push(RequestMessage::new(piece_index as u32, block_offset as u32, block_size));
        }

        // Check for any last smaller block within the current piece
        let partial_block_length = next_piece_len % block_size as u64;
        if partial_block_length != 0 {
            let block_offset = whole_blocks * block_size as u64;

            requests.push(RequestMessage::new(piece_index as u32, block_offset as u32, partial_block_length as usize));
        }

        // Take this piece out of the total length, increment to the next piece
        total_file_length -= next_piece_len;
        piece_index += 1;
    }

    requests
}