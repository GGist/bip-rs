use {ConnectedChannel};

use bip_peer::{PeerManagerBuilder, PeerInfo, IPeerManagerMessage, OPeerManagerMessage};
use bip_peer::protocols::{NullProtocol};
use bip_peer::messages::PeerWireProtocolMessage;
use bip_util::bt;
use futures::{future, Future, AsyncSink};
use futures::sink::Sink;
use futures::stream::Stream;
use tokio_core::reactor::Core;

#[test]
fn positive_peer_manager_send_backpressure() {
    let mut core = Core::new().unwrap();
    let manager = PeerManagerBuilder::new()
        .with_peer_capacity(1)
        .build(core.handle());

    // Create two peers
    let (peer_one, peer_two): (ConnectedChannel<PeerWireProtocolMessage<NullProtocol>, PeerWireProtocolMessage<NullProtocol>>,
                               ConnectedChannel<PeerWireProtocolMessage<NullProtocol>, PeerWireProtocolMessage<NullProtocol>>) = ::connected_channel(5);
    let peer_one_info = PeerInfo::new("127.0.0.1:0".parse().unwrap(), [0u8; bt::PEER_ID_LEN].into(), [0u8; bt::INFO_HASH_LEN].into());
    let peer_two_info = PeerInfo::new("127.0.0.1:1".parse().unwrap(), [1u8; bt::PEER_ID_LEN].into(), [1u8; bt::INFO_HASH_LEN].into());

    // Add peer one to the manager
    let manager = core.run(manager.send(IPeerManagerMessage::AddPeer(peer_one_info, peer_one))).unwrap();

    // Check that peer one was added
    let (response, mut manager) = core.run(manager.into_future().map(|(opt_item, stream)| (opt_item.unwrap(), stream)).map_err(|_| ())).unwrap();
    match response {
        OPeerManagerMessage::PeerAdded(info) => assert_eq!(peer_one_info, info),
        _                                    => panic!("Unexpected First Peer Manager Response")
    };

    // Try to add peer two, but make sure it was denied (start send returned not ready)
    let (response, manager) = core.run(future::lazy(|| {
        future::ok::<_, ()>((manager.start_send(IPeerManagerMessage::AddPeer(peer_two_info, peer_two)), manager))
    })).unwrap();
    let peer_two = match response {
        Ok(AsyncSink::NotReady(IPeerManagerMessage::AddPeer(info, peer_two))) => { assert_eq!(peer_two_info, info); peer_two },
        _                                                                     => panic!("Unexpected Second Peer Manager Response")
    };

    // Remove peer one from the manager
    let manager = core.run(manager.send(IPeerManagerMessage::RemovePeer(peer_one_info))).unwrap();

    // Check that peer one was removed
    let (response, manager) = core.run(manager.into_future().map(|(opt_item, stream)| (opt_item.unwrap(), stream)).map_err(|_| ())).unwrap();
    match response {
        OPeerManagerMessage::PeerRemoved(info) => assert_eq!(peer_one_info, info),
        _                                      => panic!("Unexpected Third Peer Manager Response")
    };

    // Try to add peer two, but make sure it goes through
    let manager = core.run(manager.send(IPeerManagerMessage::AddPeer(peer_two_info, peer_two))).unwrap();
    let (response, _manager) = core.run(manager.into_future().map(|(opt_item, stream)| (opt_item.unwrap(), stream)).map_err(|_| ())).unwrap();
    match response {
        OPeerManagerMessage::PeerAdded(info) => assert_eq!(peer_two_info, info),
        _                                    => panic!("Unexpected Fourth Peer Manager Response")
    };
}