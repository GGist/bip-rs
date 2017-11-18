

use ControlMessage;
use bip_handshake::InfoHash;
use bip_metainfo::Metainfo;
use bip_peer::PeerInfo;
use bip_peer::messages::{BitFieldMessage, HaveMessage};
use bit_set::BitSet;
use bytes::{BufMut, BytesMut};
use futures::{Async, AsyncSink, Sink};
use futures::Poll;
use futures::StartSend;
use futures::Stream;
use futures::task;
use futures::task::Task;
use revelation::IRevealMessage;
use revelation::ORevealMessage;
use revelation::error::{RevealError, RevealErrorKind};
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;

/// Revelation module that will honestly report any pieces we have to peers.
pub struct HonestRevealModule {
    torrents: HashMap<InfoHash, PeersInfo>,
    out_queue: VecDeque<ORevealMessage>,
    // Shared bytes container to write bitfield messages to
    out_bytes: BytesMut,
    opt_stream: Option<Task>,
}

struct PeersInfo {
    num_pieces: usize,
    status: BitSet<u8>,
    peers: HashSet<PeerInfo>,
}

impl HonestRevealModule {
    /// Create a new `HonestRevelationModule`.
    pub fn new() -> HonestRevealModule {
        HonestRevealModule {
            torrents: HashMap::new(),
            out_queue: VecDeque::new(),
            out_bytes: BytesMut::new(),
            opt_stream: None,
        }
    }

    fn add_torrent(&mut self, metainfo: &Metainfo) -> StartSend<IRevealMessage, RevealError> {
        let info_hash = metainfo.info().info_hash();

        match self.torrents.entry(info_hash) {
            Entry::Occupied(_) => {
                Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoExists { hash: info_hash }))
            },
            Entry::Vacant(vac) => {
                let num_pieces = metainfo.info().pieces().count();

                let mut piece_set = BitSet::default();
                piece_set.reserve_len_exact(num_pieces);

                let peers_info = PeersInfo {
                    num_pieces: num_pieces,
                    status: piece_set,
                    peers: HashSet::new(),
                };
                vac.insert(peers_info);

                Ok(AsyncSink::Ready)
            },
        }
    }

    fn remove_torrent(&mut self, metainfo: &Metainfo) -> StartSend<IRevealMessage, RevealError> {
        let info_hash = metainfo.info().info_hash();

        if self.torrents.remove(&info_hash).is_none() {
            Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoNotExists { hash: info_hash }))
        } else {
            Ok(AsyncSink::Ready)
        }
    }

    fn add_peer(&mut self, peer: PeerInfo) -> StartSend<IRevealMessage, RevealError> {
        let info_hash = *peer.hash();

        let out_bytes = &mut self.out_bytes;
        let out_queue = &mut self.out_queue;
        self.torrents
            .get_mut(&info_hash)
            .map(|peers_info| {
                // Add the peer to our list, so we send have messages to them
                peers_info.peers.insert(peer);

                // If our bitfield has any pieces in it, send the bitfield, otherwise, dont send it
                if !peers_info.status.is_empty() {
                    // Get our current bitfield, write it to our shared bytes
                    let bitfield_slice = peers_info.status.get_ref().storage();
                    // Bitfield stores index 0 at bit 7 from the left, we want index 0 to be at bit 0 from the left
                    insert_reversed_bits(out_bytes, bitfield_slice);

                    // Split off what we wrote, send this in the message, will be re-used on drop
                    let bitfield_bytes = out_bytes.split_off(0).freeze();
                    let bitfield = BitFieldMessage::new(bitfield_bytes);

                    // Enqueue the bitfield message so that we send it to the peer
                    out_queue.push_back(ORevealMessage::SendBitField(peer, bitfield));
                }

                Ok(AsyncSink::Ready)
            })
            .unwrap_or_else(|| Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoNotExists { hash: info_hash })))
    }

    fn remove_peer(&mut self, peer: PeerInfo) -> StartSend<IRevealMessage, RevealError> {
        let info_hash = *peer.hash();

        self.torrents
            .get_mut(&info_hash)
            .map(|peers_info| {
                peers_info.peers.remove(&peer);

                Ok(AsyncSink::Ready)
            })
            .unwrap_or_else(|| Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoNotExists { hash: info_hash })))
    }

    fn insert_piece(&mut self, hash: InfoHash, index: u64) -> StartSend<IRevealMessage, RevealError> {
        let out_queue = &mut self.out_queue;
        self.torrents
            .get_mut(&hash)
            .map(|peers_info| {
                if index as usize >= peers_info.num_pieces {
                    Err(RevealError::from_kind(RevealErrorKind::InvalidPieceOutOfRange {
                        index: index,
                        hash: hash,
                    }))
                } else {
                    // Queue up all have messages
                    for peer in peers_info.peers.iter() {
                        out_queue.push_back(ORevealMessage::SendHave(*peer, HaveMessage::new(index as u32)));
                    }

                    // Insert into bitfield
                    peers_info.status.insert(index as usize);

                    Ok(AsyncSink::Ready)
                }
            })
            .unwrap_or_else(|| Err(RevealError::from_kind(RevealErrorKind::InvalidMetainfoNotExists { hash: hash })))
    }

    //------------------------------------------------------//

    fn check_stream_unblock(&mut self) {
        if !self.out_queue.is_empty() {
            self.opt_stream.take().as_ref().map(Task::notify);
        }
    }
}

/// Inserts the slice into the `BytesMut` but reverses the bits in each byte.
fn insert_reversed_bits(bytes: &mut BytesMut, slice: &[u8]) {
    for mut byte in slice.iter().map(|byte| *byte) {
        let mut reversed_byte = 0;

        for _ in 0..8 {
            // Make room for the bit
            reversed_byte <<= 1;
            // Push the last bit over
            reversed_byte |= byte & 0x01;
            // Push the last bit off
            byte >>= 1;
        }

        bytes.put_u8(reversed_byte);
    }
}

impl Sink for HonestRevealModule {
    type SinkItem = IRevealMessage;
    type SinkError = RevealError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let result = match item {
            IRevealMessage::Control(ControlMessage::AddTorrent(metainfo)) => {
                self.add_torrent(&metainfo)
            },
            IRevealMessage::Control(ControlMessage::RemoveTorrent(metainfo)) => {
                self.remove_torrent(&metainfo)
            },
            IRevealMessage::Control(ControlMessage::PeerConnected(info)) => {
                self.add_peer(info)
            },
            IRevealMessage::Control(ControlMessage::PeerDisconnected(info)) => {
                self.remove_peer(info)
            },
            IRevealMessage::FoundGoodPiece(hash, index) => {
                self.insert_piece(hash, index)
            },
            IRevealMessage::Control(ControlMessage::Tick(_)) | IRevealMessage::ReceivedBitField(_, _) | IRevealMessage::ReceivedHave(_, _) => {
                Ok(AsyncSink::Ready)
            },
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
        let next_item = self.out_queue
            .pop_front()
            .map(|item| Ok(Async::Ready(Some(item))));

        next_item.unwrap_or_else(|| {
            self.opt_stream = Some(task::current());

            Ok(Async::NotReady)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::HonestRevealModule;
    use ControlMessage;
    use bip_handshake::Extensions;
    use bip_metainfo::{DirectAccessor, Metainfo, MetainfoBuilder, PieceLength};
    use bip_peer::PeerInfo;
    use bip_util::bt;
    use bip_util::bt::InfoHash;
    use futures::{Async, Sink, Stream};
    use futures_test::harness::Harness;
    use revelation::{IRevealMessage, ORevealMessage};
    use revelation::error::RevealErrorKind;

    fn metainfo(num_pieces: usize) -> Metainfo {
        let data = vec![0u8; num_pieces];

        let accessor = DirectAccessor::new("MyFile.txt", &data);
        let bytes = MetainfoBuilder::new()
            .set_piece_length(PieceLength::Custom(1))
            .build(1, accessor, |_| ())
            .unwrap();

        Metainfo::from_bytes(bytes).unwrap()
    }

    fn peer_info(hash: InfoHash) -> PeerInfo {
        PeerInfo::new("0.0.0.0:0".parse().unwrap(), [0u8; bt::PEER_ID_LEN].into(), hash, Extensions::new())
    }

    #[test]
    fn positive_add_and_remove_metainfo() {
        let (send, _recv) = HonestRevealModule::new().split();
        let metainfo = metainfo(1);

        let mut block_send = send.wait();

        block_send
            .send(IRevealMessage::Control(ControlMessage::AddTorrent(metainfo.clone())))
            .unwrap();
        block_send
            .send(IRevealMessage::Control(ControlMessage::RemoveTorrent(metainfo.clone())))
            .unwrap();
    }

    #[test]
    fn positive_send_bitfield_single_piece() {
        let (send, recv) = HonestRevealModule::new().split();
        let metainfo = metainfo(8);
        let info_hash = metainfo.info().info_hash();
        let peer_info = peer_info(info_hash);

        let mut block_send = send.wait();
        let mut block_recv = recv.wait();

        block_send
            .send(IRevealMessage::Control(ControlMessage::AddTorrent(metainfo)))
            .unwrap();
        block_send
            .send(IRevealMessage::FoundGoodPiece(info_hash, 0))
            .unwrap();
        block_send
            .send(IRevealMessage::Control(ControlMessage::PeerConnected(peer_info)))
            .unwrap();

        let (info, bitfield) = match block_recv.next().unwrap().unwrap() {
            ORevealMessage::SendBitField(info, bitfield) => {
                (info, bitfield)
            },
            _ => {
                panic!("Received Unexpected Message")
            },
        };

        assert_eq!(peer_info, info);
        assert_eq!(1, bitfield.bitfield().len());
        assert_eq!(0x80, bitfield.bitfield()[0]);
    }

    #[test]
    fn positive_send_bitfield_multiple_pieces() {
        let (send, recv) = HonestRevealModule::new().split();
        let metainfo = metainfo(16);
        let info_hash = metainfo.info().info_hash();
        let peer_info = peer_info(info_hash);

        let mut block_send = send.wait();
        let mut block_recv = recv.wait();

        block_send
            .send(IRevealMessage::Control(ControlMessage::AddTorrent(metainfo)))
            .unwrap();
        block_send
            .send(IRevealMessage::FoundGoodPiece(info_hash, 0))
            .unwrap();
        block_send
            .send(IRevealMessage::FoundGoodPiece(info_hash, 8))
            .unwrap();
        block_send
            .send(IRevealMessage::FoundGoodPiece(info_hash, 15))
            .unwrap();
        block_send
            .send(IRevealMessage::Control(ControlMessage::PeerConnected(peer_info)))
            .unwrap();

        let (info, bitfield) = match block_recv.next().unwrap().unwrap() {
            ORevealMessage::SendBitField(info, bitfield) => {
                (info, bitfield)
            },
            _ => {
                panic!("Received Unexpected Message")
            },
        };

        assert_eq!(peer_info, info);
        assert_eq!(2, bitfield.bitfield().len());
        assert_eq!(0x80, bitfield.bitfield()[0]);
        assert_eq!(0x81, bitfield.bitfield()[1]);
    }

    #[test]
    fn negative_dont_send_empty_bitfield() {
        let (send, recv) = HonestRevealModule::new().split();
        let metainfo = metainfo(16);
        let info_hash = metainfo.info().info_hash();
        let peer_info = peer_info(info_hash);

        let mut block_send = send.wait();
        let mut non_block_recv = Harness::new(recv);

        block_send
            .send(IRevealMessage::Control(ControlMessage::AddTorrent(metainfo)))
            .unwrap();
        block_send
            .send(IRevealMessage::Control(ControlMessage::PeerConnected(peer_info)))
            .unwrap();

        assert!(
            non_block_recv
                .poll_next()
                .as_ref()
                .map(Async::is_not_ready)
                .unwrap_or(false)
        );
    }

    #[test]
    fn negative_found_good_piece_out_of_range() {
        let (send, _recv) = HonestRevealModule::new().split();
        let metainfo = metainfo(8);
        let info_hash = metainfo.info().info_hash();

        let mut block_send = send.wait();

        block_send
            .send(IRevealMessage::Control(ControlMessage::AddTorrent(metainfo)))
            .unwrap();

        let error = block_send
            .send(IRevealMessage::FoundGoodPiece(info_hash, 8))
            .unwrap_err();
        match error.kind() {
            &RevealErrorKind::InvalidPieceOutOfRange { hash, index } => {
                assert_eq!(info_hash, hash);
                assert_eq!(8, index);
            },
            _ => {
                panic!("Received Unexpected Message")
            },
        };
    }

    #[test]
    fn negative_all_pieces_good_found_good_piece_out_of_range() {
        let (send, _recv) = HonestRevealModule::new().split();
        let metainfo = metainfo(3);
        let info_hash = metainfo.info().info_hash();

        let mut block_send = send.wait();

        block_send
            .send(IRevealMessage::Control(ControlMessage::AddTorrent(metainfo)))
            .unwrap();
        block_send
            .send(IRevealMessage::FoundGoodPiece(info_hash, 0))
            .unwrap();
        block_send
            .send(IRevealMessage::FoundGoodPiece(info_hash, 1))
            .unwrap();
        block_send
            .send(IRevealMessage::FoundGoodPiece(info_hash, 2))
            .unwrap();

        let error = block_send
            .send(IRevealMessage::FoundGoodPiece(info_hash, 3))
            .unwrap_err();
        match error.kind() {
            &RevealErrorKind::InvalidPieceOutOfRange { hash, index } => {
                assert_eq!(info_hash, hash);
                assert_eq!(3, index);
            },
            _ => {
                panic!("Received Unexpected Message")
            },
        };
    }
}
