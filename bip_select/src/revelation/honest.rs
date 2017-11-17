

use bytes::BytesMut;
use bip_metainfo::Metainfo;
use futures::task::Task;
use bip_handshake::InfoHash;
use std::collections::HashMap;
use futures::{Sink, AsyncSink, Async};
use futures::Stream;
use futures::Poll;
use futures::StartSend;
use revelation::error::{RevealError, RevealErrorKind};
use revelation::IRevealMessage;
use bip_peer::PeerInfo;
use std::collections::HashSet;
use bit_set::BitSet;
use revelation::ORevealMessage;
use std::collections::VecDeque;
use ControlMessage;
use std::collections::hash_map::Entry;
use bip_peer::messages::{BitFieldMessage, HaveMessage};
use futures::task;

/// Revelation module that will honestly report any pieces we have to peers.
pub struct HonestRevealModule {
    torrents:   HashMap<InfoHash, PeersInfo>,
    out_queue:  VecDeque<ORevealMessage>,
    // Shared bytes container to write bitfield messages to
    out_bytes:  BytesMut,
    opt_stream: Option<Task>
}

struct PeersInfo {
    status: BitSet<u8>,
    peers:  HashSet<PeerInfo>
}

impl HonestRevealModule {
    /// Create a new `HonestRevelationModule`.
    pub fn new() -> HonestRevealModule {
        HonestRevealModule{ torrents: HashMap::new(), out_queue: VecDeque::new(),
            out_bytes: BytesMut::new(), opt_stream: None }
    }

    fn add_torrent(&mut self, metainfo: &Metainfo) -> StartSend<IRevealMessage, RevealError> {
        let info_hash = metainfo.info().info_hash();

        match self.torrents.entry(info_hash) {
            Entry::Occupied(_) => {
                Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoExists{ hash: info_hash }))
            },
            Entry::Vacant(vac) => {
                let mut piece_set = BitSet::default();
                piece_set.reserve_len_exact(metainfo.info().pieces().count());

                let peers_info = PeersInfo{ status: piece_set, peers: HashSet::new() };
                vac.insert(peers_info);

                Ok(AsyncSink::Ready)
            }
        }
    }

    fn remove_torrent(&mut self, metainfo: &Metainfo) -> StartSend<IRevealMessage, RevealError> {
        let info_hash = metainfo.info().info_hash();

        if self.torrents.remove(&info_hash).is_none() {
            Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoNotExists{ hash: info_hash }))
        } else {
            Ok(AsyncSink::Ready)
        }
    }

    fn add_peer(&mut self, peer: PeerInfo) -> StartSend<IRevealMessage, RevealError> {
        let info_hash = *peer.hash();

        let out_bytes = &mut self.out_bytes;
        let out_queue = &mut self.out_queue;
        self.torrents.get_mut(&info_hash)
            .map(|peers_info| {
                // Add the peer to our list, so we send have messages to them
                peers_info.peers.insert(peer);

                // Get our current bitfield, write it to our shared bytes
                let bitfield_slice = peers_info.status.get_ref().storage();
                out_bytes.extend_from_slice(bitfield_slice);
                // Split off what we wrote, send this in the message, will be re-used on drop
                let bitfield_bytes = out_bytes.split_off(0).freeze();
                let bitfield = BitFieldMessage::new(bitfield_bytes);
                
                // Enqueue the bitfield message so that we send it to the peer
                out_queue.push_back(ORevealMessage::SendBitField(peer, bitfield));

                Ok(AsyncSink::Ready)
            }).unwrap_or_else(|| Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoNotExists{ hash: info_hash })))
    }

    fn remove_peer(&mut self, peer: PeerInfo) -> StartSend<IRevealMessage, RevealError> {
        let info_hash = *peer.hash();

        self.torrents.get_mut(&info_hash)
            .map(|peers_info| {
                peers_info.peers.remove(&peer);

                Ok(AsyncSink::Ready)
            }).unwrap_or_else(|| Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoNotExists{ hash: info_hash })))
    }

    fn insert_piece(&mut self, hash: InfoHash, index: u64) -> StartSend<IRevealMessage, RevealError> {
        let out_queue = &mut self.out_queue;
        self.torrents.get_mut(&hash)
            .map(|peers_info| {
                // Queue up all have messages
                for peer in peers_info.peers.iter() {
                    out_queue.push_back(ORevealMessage::SendHave(*peer, HaveMessage::new(index as u32)));
                }

                // Insert into bitfield
                peers_info.status.insert(index as usize);

                Ok(AsyncSink::Ready)
            }).unwrap_or_else(|| Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoNotExists{ hash: hash })))
    }

    //------------------------------------------------------//

    fn check_stream_unblock(&mut self) {
        if !self.out_queue.is_empty() {
            self.opt_stream.take().as_ref().map(Task::notify);
        }
    }
}

impl Sink for HonestRevealModule {
    type SinkItem = IRevealMessage;
    type SinkError = RevealError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let result = match item {
            IRevealMessage::Control(ControlMessage::AddTorrent(metainfo)) => self.add_torrent(&metainfo),
            IRevealMessage::Control(ControlMessage::RemoveTorrent(metainfo)) => self.remove_torrent(&metainfo),
            IRevealMessage::Control(ControlMessage::PeerConnected(info)) => self.add_peer(info),
            IRevealMessage::Control(ControlMessage::PeerDisconnected(info)) => self.remove_peer(info),
            IRevealMessage::FoundGoodPiece(hash, index) => self.insert_piece(hash, index),
            IRevealMessage::Control(ControlMessage::Tick(_)) |
            IRevealMessage::ReceivedBitField(_, _) |
            IRevealMessage::ReceivedHave(_, _) => Ok(AsyncSink::Ready)
        };

        self.check_stream_unblock();

        result
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

impl Stream for HonestRevealModule {
    type Item = ORevealMessage;
    type Error = RevealError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let next_item = self.out_queue.pop_front().map(|item| Ok(Async::Ready(Some(item))));

        next_item.unwrap_or_else(|| {
            self.opt_stream = Some(task::current());

            Ok(Async::NotReady)
        })
    }
}