use std::time::Duration;

use {MultiFileDirectAccessor, InMemoryFileSystem};
use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage, BlockManager, BlockMetadata};
use bip_metainfo::{MetainfoBuilder, PieceLength, MetainfoFile};
use bip_util::bt::InfoHash;
use tokio_core::reactor::{Core, Timeout};
use futures::future::{self, Loop, Future};
use futures::stream::Stream;
use futures::sink::{Wait, Sink};

#[test]
fn positive_complete_torrent() {
    // Create some "files" as random bytes
    let data_a = (::random_buffer(1023), "/path/to/file/a".into());
    let data_b = (::random_buffer(2000), "/path/to/file/b".into());

    // Create our accessor for our in memory files and create a torrent file for them
    let files_accessor = MultiFileDirectAccessor::new("/my/downloads/".into(),
        vec![data_a.clone(), data_b.clone()]);
    let metainfo_bytes = MetainfoBuilder::new()
        .set_piece_length(PieceLength::Custom(1024))
        .build_as_bytes(1, files_accessor, |_| ()).unwrap();
    let metainfo_file = MetainfoFile::from_bytes(metainfo_bytes).unwrap();

    // Spin up a disk manager and add our created torrent to it
    let filesystem = InMemoryFileSystem::new();
    let disk_manager = DiskManagerBuilder::new()
        .build(filesystem.clone());

    let (send, recv) = disk_manager.split();
    let mut blocking_send = send.wait();
    blocking_send.send(IDiskMessage::AddTorrent(metainfo_file.clone())).unwrap();

    // Verify that zero pieces are marked as good
    let mut core = Core::new().unwrap();
    let timeout = Timeout::new(Duration::from_millis(100), &core.handle()).unwrap()
        .then(|_| Err(()));
    let (good_pieces, recv) = core.run(
        future::loop_fn((0, recv), |(good, recv)| {
            recv.into_future()
            .map(move |(opt_msg, recv)| {
                match opt_msg.unwrap() {
                    ODiskMessage::TorrentAdded(_)      => Loop::Break((good, recv)),
                    ODiskMessage::FoundGoodPiece(_, _) => Loop::Continue((good + 1, recv)),
                    unexpected @ _                     => panic!("Unexpected Message: {:?}", unexpected)
                }
            })
        })
        .map_err(|_| ())
        .select(timeout)
        .map(|(item, _)| item)
    ).unwrap_or_else(|_| panic!("Add Torrent Operation Failed Or Timed Out"));

    // Make sure we have no good pieces
    assert_eq!(0, good_pieces);

    // Send a couple blocks that are known to be good, then one bad block
    let mut files_bytes = Vec::new();
    files_bytes.extend_from_slice(&data_a.0);
    files_bytes.extend_from_slice(&data_b.0);

    // Send piece 0 with good blocks
    send_block(&mut blocking_send, &files_bytes[0..500], metainfo_file.info_hash(), 0, 0, 500, |_| ());
    send_block(&mut blocking_send, &files_bytes[500..1000], metainfo_file.info_hash(), 0, 500, 500, |_| ());
    send_block(&mut blocking_send, &files_bytes[1000..1024], metainfo_file.info_hash(), 0, 1000, 24, |_| ());

    // Verify that 

    // Send piece 1 with good blocks
    send_block(&mut blocking_send, &files_bytes[(1024 + 0)..(1024 + 500)], metainfo_file.info_hash(), 1, 0, 500, |_| ());
    send_block(&mut blocking_send, &files_bytes[(1024 + 500)..(1024 + 1000)], metainfo_file.info_hash(), 1, 500, 500, |_| ());
    send_block(&mut blocking_send, &files_bytes[(1024 + 1000)..(1024 + 1024)], metainfo_file.info_hash(), 1, 1000, 24, |_| ());

    // Send piece 2 with good blocks
    send_block(&mut blocking_send, &files_bytes[(2048 + 0)..(2048 + 500)], metainfo_file.info_hash(), 2, 0, 500, |_| ());
    send_block(&mut blocking_send, &files_bytes[(2048 + 500)..(2048 + 975)], metainfo_file.info_hash(), 2, 500, 475, |_| ());

    // Verify that piece 0 is bad, but piece 1 and 2 are good
    let timeout = Timeout::new(Duration::from_millis(100), &core.handle()).unwrap()
        .then(|_| Err(()));
    let (recv, piece_zero_good, piece_one_good, piece_two_good) = core.run(
        future::loop_fn((recv, false, false, false, 0), |(recv, piece_zero_good, piece_one_good, piece_two_good, messages_recvd)| {
            let messages_recvd = messages_recvd + 1;

            recv.into_future()
            .map(move |(opt_msg, recv)| {
                match opt_msg.unwrap() {
                    ODiskMessage::FoundGoodPiece(_, index) => {
                        match index {
                            0 => (recv, true, piece_one_good, piece_two_good),
                            1 => (recv, piece_zero_good, true, piece_two_good),
                            2 => (recv, piece_zero_good, piece_one_good, true),
                            _ => panic!("Unexpected FoundGoodPiece Index")
                        }
                    },
                    ODiskMessage::FoundBadPiece(_, index) => {
                        match index {
                            0 => (recv, false, piece_one_good, piece_two_good),
                            1 => (recv, piece_zero_good, false, piece_two_good),
                            2 => (recv, piece_zero_good, piece_one_good, false),
                            _ => panic!("Unexpected FoundBadPiece Index")
                        }
                    },
                    ODiskMessage::BlockProcessed(_) => (recv, piece_zero_good, piece_one_good, piece_two_good),
                    unexpected @ _ => panic!("Unexpected Message: {:?}", unexpected)
                }
            })
            .map(move |(recv, piece_zero_good, piece_one_good, piece_two_good)| {
                // One message for each block (8 blocks), plus 3 messages for bad/good
                if messages_recvd == (8 + 3) {
                    Loop::Break((recv, piece_zero_good, piece_one_good, piece_two_good))
                } else {
                    Loop::Continue((recv, piece_zero_good, piece_one_good, piece_two_good, messages_recvd))
                }
            })
        })
        .map_err(|_| ())
        .select(timeout)
        .map(|(item, _)| item)
    ).unwrap_or_else(|_| panic!("Found(.*)Piece Operation Failed Or Timed Out"));
    
    // Assert whether or not pieces were good
    assert_eq!(false, piece_zero_good);
    assert_eq!(true, piece_one_good);
    assert_eq!(true, piece_two_good);

    // Resend piece 0 with good blocks
    send_block(&mut blocking_send, &files_bytes[0..500], metainfo_file.info_hash(), 0, 0, 500, |_| ());
    send_block(&mut blocking_send, &files_bytes[500..1000], metainfo_file.info_hash(), 0, 500, 500, |_| ());
    send_block(&mut blocking_send, &files_bytes[1000..1024], metainfo_file.info_hash(), 0, 1000, 24, |_| ());

    /// Verify that piece 0 is now good
    let timeout = Timeout::new(Duration::from_millis(100), &core.handle()).unwrap()
        .then(|_| Err(()));
    let piece_zero_good = core.run(
        future::loop_fn((recv, false, 0), |(recv, piece_zero_good, messages_recvd)| {
            let messages_recvd = messages_recvd + 1;

            recv.into_future()
            .map(move |(opt_msg, recv)| {
                match opt_msg.unwrap() {
                    ODiskMessage::FoundGoodPiece(_, index) => {
                        match index {
                            0 => (recv, true),
                            _ => panic!("Unexpected FoundGoodPiece Index")
                        }
                    },
                    ODiskMessage::FoundBadPiece(_, index) => {
                        match index {
                            0 => (recv, false),
                            _ => panic!("Unexpected FoundBadPiece Index")
                        }
                    },
                    ODiskMessage::BlockProcessed(_) => (recv, piece_zero_good),
                    unexpected @ _ => panic!("Unexpected Message: {:?}", unexpected)
                }
            })
            .map(move |(recv, piece_zero_good)| {
                // One message for each block (3 blocks), plus 1 messages for bad/good
                if messages_recvd == (3 + 1) {
                    Loop::Break(piece_zero_good)
                } else {
                    Loop::Continue((recv, piece_zero_good, messages_recvd))
                }
            })
        })
        .map_err(|_| ())
        .select(timeout)
        .map(|(item, _)| item)
    ).unwrap_or_else(|_| panic!("Found(.*)Piece Operation Failed Or Timed Out"));

    // Assert whether or not piece was good
    assert_eq!(true, piece_zero_good);
}

/// Send block with the given metadata and entire data given.
fn send_block<S, M>(blocking_send: &mut Wait<S>, data: &[u8], hash: InfoHash, piece_index: u64, block_offset: u64, block_len: usize, modify: M)
    where S: Sink<SinkItem=IDiskMessage>, M: Fn(&mut [u8]) {
    let mut block_manager = BlockManager::new(1, block_len).wait();

    let mut block = block_manager.next().unwrap().unwrap();
    block.set_metadata(BlockMetadata::new(hash, piece_index, block_offset, block_len));

    (&mut block[..block_len]).copy_from_slice(data);

    modify(&mut block[..]);

    blocking_send.send(IDiskMessage::ProcessBlock(block)).unwrap_or_else(|_| panic!("Failed To Send Process Block Message"));
}