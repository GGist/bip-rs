extern crate bip_handshake;
extern crate bip_metainfo;
extern crate bip_peer;
#[macro_use]
extern crate clap;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate hex;

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

//use bip_dht::{DhtBuilder, Handshaker, Router};
use bip_handshake::{HandshakerBuilder, InitiateMessage, Protocol, HandshakerConfig, InfoHash, Extensions, Extension};
use bip_handshake::transports::TcpTransport;
use bip_peer::{PeerManagerBuilder, IPeerManagerMessage, PeerInfo, PeerProtocolCodec, OPeerManagerMessage};
use bip_peer::protocols::{PeerWireProtocol, NullProtocol, PeerExtensionProtocol};
use bip_peer::message::{PeerWireProtocolMessage, ExtendedMessage, UtMetadataMessage, BitsExtensionMessage,
                        PeerExtensionProtocolMessage, ExtendedType, UtMetadataRequestMessage};
use bip_metainfo::{Info, Metainfo};
use tokio_core::reactor::Core;
use tokio_io::{AsyncRead,};
use futures::{future, stream, Future, Stream, Sink};
use futures::sync::mpsc;
use futures::future::{Loop};
use hex::FromHex;

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

const MAX_DATA_BLOCK_SIZE: usize = 16 * 1024;

// Some enum to store our selection state updates
#[derive(Debug)]
enum SelectState {
    Choke(PeerInfo),
    UnChoke(PeerInfo),
    Interested(PeerInfo),
    UnInterested(PeerInfo),
    NewPeer(PeerInfo),
    RemovedPeer(PeerInfo),
    Extended(ExtendedMessage),
    UtMetadata(UtMetadataMessage)
}

fn main() {
    // Command line argument parsing
    let matches = clap_app!(myapp =>
        (version: "1.0")
        (author: "Andrew <amiller4421@gmail.com>")
        (about: "Download torrent file from info hash")
        (@arg hash: -h +required +takes_value "InfoHash of the torrent")
        (@arg peer: -p +required +takes_value "Single peer to connect to of the form addr:port")
        (@arg output: -f +required +takes_value "Output to write the torrent file to")
    ).get_matches();
    let hash = matches.value_of("hash").unwrap();
    let peer_addr = matches.value_of("peer").unwrap().parse().unwrap();
    let output = matches.value_of("output").unwrap();

    let hash: Vec<u8> = FromHex::from_hex(hash).unwrap();
    let info_hash = InfoHash::from_hash(&hash[..]).unwrap();

    // Create our main "core" event loop
    let mut core = Core::new().unwrap();
    
    // Activate the extension protocol via the handshake bits (we will send an extended message if they support it...)
    let mut extensions = Extensions::new();
    extensions.add(Extension::ExtensionProtocol);

    // Create a handshaker that can initiate connections with peers
    let (handshaker_send, handshaker_recv) = HandshakerBuilder::new()
        .with_config(HandshakerConfig::default()
            .with_wait_buffer_size(0)
            .with_done_buffer_size(0))
        .with_extensions(extensions)
        .build::<TcpTransport>(core.handle()) // Will handshake over TCP (could swap this for UTP in the future)
        .unwrap()
        .into_parts();
    // Create a peer manager that will hold our peers and heartbeat/send messages to them
    let (peer_manager_send, peer_manager_recv) = PeerManagerBuilder::new()
        // Similar to the disk manager sink and stream capacities, we can constrain those
        // for the peer manager as well.
        .with_sink_buffer_capacity(0)
        .with_stream_buffer_capacity(0)
        .build(core.handle())
        .into_parts();

    // Hook up a future that feeds incoming (handshaken) peers over to the peer manager
    let map_peer_manager_send = peer_manager_send.clone().sink_map_err(|_| ());
    core.handle().spawn(handshaker_recv
        .map_err(|_| ())
        .map(|complete_msg| {
            // Our handshaker finished handshaking some peer, get
            // the peer info as well as the peer itself (socket)
            let (_, extensions, hash, pid, addr, sock) = complete_msg.into_parts();

            // Only connect to peer that support the extension protocol...
            if extensions.contains(Extension::ExtensionProtocol) {
                // Frame our socket with the peer wire protocol with no extensions (nested null protocol), and a max payload of 24KB
                let peer = sock.framed(PeerProtocolCodec::with_max_payload(PeerWireProtocol::new(PeerExtensionProtocol::new(NullProtocol::new())), 24 * 1024));
                
                // Create our peer identifier used by our peer manager
                let peer_info = PeerInfo::new(addr, pid, hash);

                // Map to a message that can be fed to our peer manager
                IPeerManagerMessage::AddPeer(peer_info, peer)
            } else {
                panic!("Chosen Peer Does Not Support Extended Messages")
            }
        })
        .forward(map_peer_manager_send)
        .map(|_| ())
    );

    // Will hold a mapping of BlockMetadata -> Vec<PeerInfo> to track which peers to send a queued block to
    let (select_send, select_recv) = mpsc::channel(50);

    // Map out the errors for these sinks so they match
    let map_select_send = select_send.clone().sink_map_err(|_| ());

    // Hook up a future that receives messages from the peer manager, and forwards request to the disk manager or selection manager (using loop fn
    // here because we need to be able to access state, like request_map and a different future combinator wouldnt let us keep it around to access)
    core.handle().spawn(future::loop_fn((peer_manager_recv, info_hash, map_select_send), |(peer_manager_recv, info_hash, select_send)| {
        peer_manager_recv.into_future()
            .map_err(|_| ())
            .and_then(move |(opt_item, peer_manager_recv)| {
                let opt_message = match opt_item.unwrap() {
                    OPeerManagerMessage::ReceivedMessage(info, message) => {
                        match message {
                            PeerWireProtocolMessage::Choke                                                                => Some(SelectState::Choke(info)),
                            PeerWireProtocolMessage::UnChoke                                                              => Some(SelectState::UnChoke(info)),
                            PeerWireProtocolMessage::Interested                                                           => Some(SelectState::Interested(info)),
                            PeerWireProtocolMessage::UnInterested                                                         => Some(SelectState::UnInterested(info)),
                            PeerWireProtocolMessage::BitsExtension(BitsExtensionMessage::Extended(extended))              => Some(SelectState::Extended(extended)),
                            PeerWireProtocolMessage::ProtExtension(PeerExtensionProtocolMessage::UtMetadata(ut_metadata)) => Some(SelectState::UtMetadata(ut_metadata)),
                            _                                                                                             => None
                        }
                    },
                    OPeerManagerMessage::PeerAdded(info)        => Some(SelectState::NewPeer(info)),
                    OPeerManagerMessage::SentMessage(_, _)      => None,
                    OPeerManagerMessage::PeerRemoved(info)      => { println!("We Removed Peer {:?} From The Peer Manager", info); Some(SelectState::RemovedPeer(info)) },
                    OPeerManagerMessage::PeerDisconnect(info)   => { println!("Peer {:?} Disconnected From Us", info); Some(SelectState::RemovedPeer(info)) },
                    OPeerManagerMessage::PeerError(info, error) => { println!("Peer {:?} Disconnected With Error: {:?}", info, error); Some(SelectState::RemovedPeer(info)) }
                };

                // Could optimize out the box, but for the example, this is cleaner and shorter
                let result_future: Box<Future<Item=Loop<(), _>, Error=()>> = match opt_message {
                    Some(select_message) =>
                        Box::new(select_send.send(select_message).map(move |select_send| Loop::Continue((peer_manager_recv, info_hash, select_send)))),
                    None                            =>
                        Box::new(future::ok(Loop::Continue((peer_manager_recv, info_hash, select_send))))
                };

                result_future
            })
    }));

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

    let result: Result<_, ()> = core.run(future::loop_fn((select_recv, map_peer_manager_send, None, false, None, Vec::new()),
    |(select_recv, map_peer_manager_send, mut opt_peer, mut unchoked, mut opt_responses_expected, mut responses)| {
        select_recv.into_future()
            .map_err(|_| ())
            .and_then(move |(opt_message, select_recv)| {
                let send_messages = match opt_message.unwrap() {
                    SelectState::Choke(_) => {
                        unchoked = false;
                        vec![]
                    },
                    SelectState::UnChoke(_) => {
                        unchoked = true;
                        vec![]
                    },
                    SelectState::NewPeer(info) => {
                        // Create an ad-hoc extended message...
                        let mut id_map = HashMap::new();
                        id_map.insert(ExtendedType::UtMetadata, 1);
                        let extended_msg = ExtendedMessage::new(id_map, None, None, None, None, None, None, None);

                        opt_peer = Some(info);
                        vec![IPeerManagerMessage::SendMessage(info, 0, PeerWireProtocolMessage::BitsExtension(BitsExtensionMessage::Extended(extended_msg))),
                             IPeerManagerMessage::SendMessage(info, 0, PeerWireProtocolMessage::Interested),
                             IPeerManagerMessage::SendMessage(info, 0, PeerWireProtocolMessage::UnChoke)]
                    },
                    SelectState::Extended(extended) => {
                        // Check if they support the ut metadata extension
                        match (extended.query_id(&ExtendedType::UtMetadata), extended.metadata_size()) {
                            (Some(_), Some(size)) => {
                                let num_requests = if (size as usize) % MAX_DATA_BLOCK_SIZE != 0 {
                                    (size as usize) / MAX_DATA_BLOCK_SIZE + 1
                                } else {
                                    (size as usize) / MAX_DATA_BLOCK_SIZE
                                };
                                
                                let mut requests = Vec::new();
                                for index in 0..num_requests {
                                    requests.push(IPeerManagerMessage::SendMessage(opt_peer.unwrap(), 0,
                                        PeerWireProtocolMessage::ProtExtension(
                                            PeerExtensionProtocolMessage::UtMetadata(
                                                UtMetadataMessage::Request(
                                                    UtMetadataRequestMessage::new(index as i64))))));
                                }
                                opt_responses_expected = Some(requests.len());

                                requests
                            },
                            (_, _) => {
                                panic!("Chosen Peer Does Not Support UtMetadata/Has No Metadata")
                            }
                        }
                    },
                    SelectState::UtMetadata(ut_metadata) => {
                        match ut_metadata {
                            UtMetadataMessage::Request(_) => (),
                            UtMetadataMessage::Data(data) => responses.push(data),
                            UtMetadataMessage::Reject(_)  => panic!("Peer Rejected Metadata Message")
                        }
                        vec![]
                    },
                    SelectState::RemovedPeer(info) => panic!("Peer {:?} Got Disconnected", info),
                    _                              => vec![]
                };
                
                let result: Box<Future<Item=Loop<_,_>,Error=()>> = if opt_responses_expected == Some(responses.len()) {
                    Box::new(future::ok(Loop::Break(responses)))
                } else {
                    Box::new(map_peer_manager_send
                        .send_all(stream::iter(send_messages.into_iter().map(Ok::<_, ()>)))
                        .map_err(|_| ())
                        .map(move |(map_peer_manager_send, _)| {
                            Loop::Continue((select_recv, map_peer_manager_send, opt_peer, unchoked, opt_responses_expected, responses))
                        })
                    )
                };

                result
            })
    }));
    
    let mut messages = result.unwrap();
    messages.sort_by(|a, b| a.piece().cmp(&b.piece()));

    let mut info_bytes = Vec::new();
    for message in messages {
        info_bytes.extend_from_slice(message.data().as_ref());
    }

    let metainfo: Metainfo = Info::from_bytes(info_bytes).unwrap().into();

    let mut file = File::create(output).unwrap();
    file.write_all(&metainfo.to_bytes()[..]).unwrap();
}