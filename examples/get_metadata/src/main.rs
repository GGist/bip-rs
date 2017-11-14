extern crate bip_dht;
extern crate bip_handshake;
extern crate bip_metainfo;
extern crate bip_peer;
extern crate bip_select;
#[macro_use]
extern crate clap;
extern crate futures;
extern crate hex;
extern crate tokio_core;
extern crate tokio_io;
extern crate pendulum;

use bip_dht::{DhtBuilder, DhtEvent, Handshaker, Router};
use bip_handshake::{Extension, Extensions, HandshakerBuilder, HandshakerConfig, InfoHash, InitiateMessage, Protocol};
use bip_handshake::DiscoveryInfo;
use bip_handshake::PeerId;
use bip_handshake::transports::TcpTransport;
use bip_peer::{IPeerManagerMessage, OPeerManagerMessage, PeerInfo, PeerManagerBuilder, PeerProtocolCodec};
use bip_peer::messages::{BitsExtensionMessage, PeerExtensionProtocolMessage, PeerWireProtocolMessage};
use bip_peer::messages::builders::ExtendedMessageBuilder;
use bip_peer::protocols::{NullProtocol, PeerExtensionProtocol, PeerWireProtocol};
use bip_select::{ControlMessage, IDiscoveryMessage, IExtendedMessage, IUberMessage, ODiscoveryMessage, OExtendedMessage, OUberMessage,
                 UberModuleBuilder, UtMetadataModule};
use futures::{Future, Sink, Stream};
use futures::future::{self, Either, Loop};
use futures::sink::Wait;
use hex::FromHex;
use std::fmt::Debug;
use std::fs::File;
use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;
use pendulum::{HashedWheelBuilder};
use pendulum::future::{TimerBuilder};

// Legacy Handshaker, when bip_dht is migrated, it will accept S directly
struct LegacyHandshaker<S> {
    port: u16,
    id: PeerId,
    sender: Wait<S>,
}

impl<S> LegacyHandshaker<S>
where
    S: DiscoveryInfo + Sink,
{
    pub fn new(sink: S) -> LegacyHandshaker<S> {
        LegacyHandshaker {
            port: sink.port(),
            id: sink.peer_id(),
            sender: sink.wait(),
        }
    }
}

impl<S> Handshaker for LegacyHandshaker<S>
where
    S: Sink<SinkItem = InitiateMessage> + Send,
    S::SinkError: Debug,
{
    type MetadataEnvelope = ();

    fn id(&self) -> PeerId {
        self.id
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn connect(&mut self, _expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr) {
        self.sender
            .send(InitiateMessage::new(Protocol::BitTorrent, hash, addr))
            .unwrap();
    }

    fn metadata(&mut self, _data: ()) {
        ()
    }
}

fn main() {
    let matches = clap_app!(myapp =>
        (version: "1.0")
        (author: "Andrew <amiller4421@gmail.com>")
        (about: "Download torrent file from info hash")
        (@arg hash: -h +required +takes_value "InfoHash of the torrent")
        //(@arg peer: -p +required +takes_value "Single peer to connect to of the form addr:port")
        (@arg output: -f +required +takes_value "Output to write the torrent file to")
    ).get_matches();
    let hash = matches.value_of("hash").unwrap();
    //let addr = matches.value_of("peer").unwrap().parse().unwrap();
    let output = matches.value_of("output").unwrap();

    let hash: Vec<u8> = FromHex::from_hex(hash).unwrap();
    let info_hash = InfoHash::from_hash(&hash[..]).unwrap();

    // Create our main "core" event loop
    let mut core = Core::new().unwrap();

    // Activate the extension protocol via the handshake bits
    let mut extensions = Extensions::new();
    extensions.add(Extension::ExtensionProtocol);

    // Create a handshaker that can initiate connections with peers
    let (handshaker_send, handshaker_recv) = HandshakerBuilder::new()
        .with_extensions(extensions)
        .with_config(
            // Set a low handshake timeout so we dont wait on peers that arent listening on tcp
            HandshakerConfig::default().with_connect_timeout(Duration::from_millis(500)),
        )
        .build(TcpTransport, core.handle())
        .unwrap()
        .into_parts();
    // Create a peer manager that will hold our peers and heartbeat/send messages to them
    let (peer_manager_send, peer_manager_recv) = PeerManagerBuilder::new().build(core.handle()).into_parts();

    // Hook up a future that feeds incoming (handshaken) peers over to the peer manager
    core.handle().spawn(
        handshaker_recv
            .map_err(|_| ())
            .map(|complete_msg| {
                // Our handshaker finished handshaking some peer, get
                // the peer info as well as the peer itself (socket)
                let (_, extensions, hash, pid, addr, sock) = complete_msg.into_parts();

                // Only connect to peer that support the extension protocol...
                if extensions.contains(Extension::ExtensionProtocol) {
                    // Frame our socket with the peer wire protocol with no
                    // extensions (nested null protocol), and a max payload of 24KB
                    let peer = sock.framed(PeerProtocolCodec::with_max_payload(
                        PeerWireProtocol::new(PeerExtensionProtocol::new(NullProtocol::new())),
                        24 * 1024,
                    ));

                    // Create our peer identifier used by our peer manager
                    let peer_info = PeerInfo::new(addr, pid, hash, extensions);

                    // Map to a message that can be fed to our peer manager
                    IPeerManagerMessage::AddPeer(peer_info, peer)
                } else {
                    panic!("Chosen Peer Does Not Support Extended Messages")
                }
            })
            .forward(peer_manager_send.clone().sink_map_err(|_| ()))
            .map(|_| ()),
    );

    // Create our UtMetadata selection module
    let (uber_send, uber_recv) = UberModuleBuilder::new()
        .with_extended_builder(Some(ExtendedMessageBuilder::new()))
        .with_discovery_module(UtMetadataModule::new())
        .build()
        .split();

    // Tell the uber module we want to download metainfo for the given hash
    let uber_send = core.run(
        uber_send
            .send(IUberMessage::Discovery(IDiscoveryMessage::DownloadMetainfo(info_hash)))
            .map_err(|_| ()),
    ).unwrap();

    let timer = TimerBuilder::default()
        .build(HashedWheelBuilder::default().build());
    let timer_recv = timer.sleep_stream(Duration::from_millis(100))
        .unwrap()
        .map(Either::B);

    let merged_recv = peer_manager_recv
        .map(Either::A)
        .map_err(|_| ())
        .select(timer_recv);

    // Hook up a future that receives messages from the peer manager
    core.handle().spawn(future::loop_fn(
        (merged_recv, info_hash, uber_send.sink_map_err(|_| ())),
        |(merged_recv, info_hash, select_send)| {
            merged_recv
                .into_future()
                .map_err(|_| ())
                .and_then(move |(opt_item, merged_recv)| {
                    let opt_message = match opt_item.unwrap() {
                        Either::A(
                            OPeerManagerMessage::ReceivedMessage(
                                info,
                                PeerWireProtocolMessage::BitsExtension(BitsExtensionMessage::Extended(extended)),
                            ),
                        ) => {
                            Some(IUberMessage::Extended(IExtendedMessage::RecievedExtendedMessage(info, extended)))
                        },
                        Either::A(
                            OPeerManagerMessage::ReceivedMessage(
                                info,
                                PeerWireProtocolMessage::ProtExtension(PeerExtensionProtocolMessage::UtMetadata(message)),
                            ),
                        ) => {
                            Some(IUberMessage::Discovery(IDiscoveryMessage::ReceivedUtMetadataMessage(info, message)))
                        },
                        Either::A(OPeerManagerMessage::PeerAdded(info)) => {
                            println!("Connected To Peer: {:?}", info);
                            Some(IUberMessage::Control(ControlMessage::PeerConnected(info)))
                        },
                        Either::A(OPeerManagerMessage::PeerRemoved(info)) => {
                            println!("We Removed Peer {:?} From The Peer Manager", info);
                            Some(IUberMessage::Control(ControlMessage::PeerDisconnected(info)))
                        },
                        Either::A(OPeerManagerMessage::PeerDisconnect(info)) => {
                            println!("Peer {:?} Disconnected From Us", info);
                            Some(IUberMessage::Control(ControlMessage::PeerDisconnected(info)))
                        },
                        Either::A(OPeerManagerMessage::PeerError(info, error)) => {
                            println!("Peer {:?} Disconnected With Error: {:?}", info, error);
                            Some(IUberMessage::Control(ControlMessage::PeerDisconnected(info)))
                        },
                        Either::B(_) => {
                            Some(IUberMessage::Control(ControlMessage::Tick(Duration::from_millis(100))))
                        },
                        _ => {
                            None
                        },
                    };

                    match opt_message {
                        Some(message) => {
                            Either::A(
                                select_send
                                    .send(message)
                                    .map(move |select_send| Loop::Continue((merged_recv, info_hash, select_send))),
                            )
                        },
                        None => {
                            Either::B(future::ok(Loop::Continue((merged_recv, info_hash, select_send))))
                        },
                    }
                })
        },
    ));

    // Setup the dht which will be the only peer discovery service we use in this example
    let legacy_handshaker = LegacyHandshaker::new(handshaker_send);
    let dht = DhtBuilder::with_router(Router::uTorrent)
        .set_read_only(false)
        .start_mainline(legacy_handshaker)
        .unwrap();

    println!("Bootstrapping Dht...");
    for message in dht.events() {
        if let DhtEvent::BootstrapCompleted = message {
            break;
        }
    }
    println!("Bootstrap Complete...");

    dht.search(info_hash, true);

    /*
    // Send the peer given from the command line over to the handshaker to initiate a connection
    core.run(
        handshaker_send
            .send(InitiateMessage::new(Protocol::BitTorrent, info_hash, addr))
            .map_err(|_| ()),
    ).unwrap();
*/

    let metainfo = core.run(future::loop_fn(
        (uber_recv, peer_manager_send.sink_map_err(|_| ()), None),
        |(select_recv, map_peer_manager_send, mut opt_metainfo)| {
            select_recv
                .into_future()
                .map_err(|_| ())
                .and_then(move |(opt_message, select_recv)| {
                    let opt_message = opt_message.and_then(|message| match message {
                        OUberMessage::Extended(OExtendedMessage::SendExtendedMessage(info, ext_message)) => {
                            Some(IPeerManagerMessage::SendMessage(
                                info,
                                0,
                                PeerWireProtocolMessage::BitsExtension(BitsExtensionMessage::Extended(ext_message)),
                            ))
                        },
                        OUberMessage::Discovery(ODiscoveryMessage::SendUtMetadataMessage(info, message)) => {
                            Some(IPeerManagerMessage::SendMessage(
                                info,
                                0,
                                PeerWireProtocolMessage::ProtExtension(PeerExtensionProtocolMessage::UtMetadata(message)),
                            ))
                        },
                        OUberMessage::Discovery(ODiscoveryMessage::DownloadedMetainfo(metainfo)) => {
                            opt_metainfo = Some(metainfo);
                            None
                        },
                        _ => {
                            panic!("Unexpected Message For Uber Module...")
                        },
                    });

                    match (opt_message, opt_metainfo.take()) {
                        (Some(message), _) => {
                            Either::A(
                                map_peer_manager_send
                                    .send(message)
                                    .map(move |peer_manager_send| Loop::Continue((select_recv, peer_manager_send, opt_metainfo))),
                            )
                        },
                        (None, None) => {
                            Either::B(future::ok(Loop::Continue((select_recv, map_peer_manager_send, opt_metainfo))))
                        },
                        (None, Some(metainfo)) => {
                            Either::B(future::ok(Loop::Break(metainfo)))
                        },
                    }
                })
        },
    )).unwrap();

    // Write the metainfo file out to the user provided path
    File::create(output)
        .unwrap()
        .write_all(&metainfo.to_bytes())
        .unwrap();
}
