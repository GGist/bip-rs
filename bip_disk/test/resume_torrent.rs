use crate::{InMemoryFileSystem, MultiFileDirectAccessor};
use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage};
use bip_metainfo::{Metainfo, MetainfoBuilder, PieceLength};
use futures::future::Loop;
use futures::sink::Sink;
use futures::stream::Stream;
use tokio_core::reactor::Core;

#[test]
fn positive_complete_torrent() {
    // Create some "files" as random bytes
    let data_a = (crate::random_buffer(1023), "/path/to/file/a".into());
    let data_b = (crate::random_buffer(2000), "/path/to/file/b".into());

    // Create our accessor for our in memory files and create a torrent file for them
    let files_accessor = MultiFileDirectAccessor::new("/my/downloads/".into(), vec![data_a.clone(), data_b.clone()]);
    let metainfo_bytes = MetainfoBuilder::new()
        .set_piece_length(PieceLength::Custom(1024))
        .build(1, files_accessor, |_| ())
        .unwrap();
    let metainfo_file = Metainfo::from_bytes(metainfo_bytes).unwrap();
    let info_hash = metainfo_file.info().info_hash();

    // Spin up a disk manager and add our created torrent to it
    let filesystem = InMemoryFileSystem::new();
    let disk_manager = DiskManagerBuilder::new().build(filesystem);

    let (send, recv) = disk_manager.split();
    let mut blocking_send = send.wait();
    blocking_send.send(IDiskMessage::AddTorrent(metainfo_file.clone())).unwrap();

    // Verify that zero pieces are marked as good
    let mut core = Core::new().unwrap();

    // Run a core loop until we get the TorrentAdded message
    let (good_pieces, recv) = crate::core_loop_with_timeout(&mut core, 500, (0, recv), |good_pieces, recv, msg| match msg {
        ODiskMessage::TorrentAdded(_) => Loop::Break((good_pieces, recv)),
        ODiskMessage::FoundGoodPiece(_, _) => Loop::Continue((good_pieces + 1, recv)),
        unexpected => panic!("Unexpected Message: {:?}", unexpected),
    });

    // Make sure we have no good pieces
    assert_eq!(0, good_pieces);

    // Send a couple blocks that are known to be good, then one bad block
    let mut files_bytes = Vec::new();
    files_bytes.extend_from_slice(&data_a.0);
    files_bytes.extend_from_slice(&data_b.0);

    // Send piece 0
    crate::send_block(
        &mut blocking_send,
        &files_bytes[0..500],
        metainfo_file.info().info_hash(),
        0,
        0,
        500,
        |_| (),
    );
    crate::send_block(
        &mut blocking_send,
        &files_bytes[500..1000],
        metainfo_file.info().info_hash(),
        0,
        500,
        500,
        |_| (),
    );
    crate::send_block(
        &mut blocking_send,
        &files_bytes[1000..1024],
        metainfo_file.info().info_hash(),
        0,
        1000,
        24,
        |_| (),
    );

    // Verify that piece 0 is good
    let (recv, piece_zero_good) =
        crate::core_loop_with_timeout(&mut core, 500, ((false, 0), recv), |(piece_zero_good, messages_recvd), recv, msg| {
            let messages_recvd = messages_recvd + 1;

            // Map BlockProcessed to a None piece index so we don't update our state
            let (opt_piece_index, new_value) = match msg {
                ODiskMessage::FoundGoodPiece(_, index) => (Some(index), true),
                ODiskMessage::FoundBadPiece(_, index) => (Some(index), false),
                ODiskMessage::BlockProcessed(_) => (None, false),
                unexpected => panic!("Unexpected Message: {:?}", unexpected),
            };

            let piece_zero_good = match opt_piece_index {
                None => piece_zero_good,
                Some(0) => new_value,
                Some(x) => panic!("Unexpected Index {:?}", x),
            };

            if messages_recvd == (3 + 1) {
                Loop::Break((recv, piece_zero_good))
            } else {
                Loop::Continue(((piece_zero_good, messages_recvd), recv))
            }
        });

    // Assert whether or not pieces were good
    assert_eq!(true, piece_zero_good);

    // Remove the torrent from our manager
    blocking_send.send(IDiskMessage::RemoveTorrent(info_hash)).unwrap();

    // Verify that our torrent was removed
    let recv = crate::core_loop_with_timeout(&mut core, 500, ((), recv), |_, recv, msg| match msg {
        ODiskMessage::TorrentRemoved(_) => Loop::Break(recv),
        unexpected => panic!("Unexpected Message: {:?}", unexpected),
    });

    // Re-add our torrent and verify that we see our good first block
    blocking_send.send(IDiskMessage::AddTorrent(metainfo_file.clone())).unwrap();

    let (recv, piece_zero_good) = crate::core_loop_with_timeout(&mut core, 500, (false, recv), |piece_zero_good, recv, msg| match msg {
        ODiskMessage::TorrentAdded(_) => Loop::Break((recv, piece_zero_good)),
        ODiskMessage::FoundGoodPiece(_, piece) if piece == 0 => Loop::Continue((true, recv)),
        unexpected => panic!("Unexpected Message: {:?}", unexpected),
    });

    assert_eq!(true, piece_zero_good);

    // Send piece 1
    crate::send_block(
        &mut blocking_send,
        &files_bytes[(1024 + 0)..(1024 + 500)],
        metainfo_file.info().info_hash(),
        1,
        0,
        500,
        |_| (),
    );
    crate::send_block(
        &mut blocking_send,
        &files_bytes[(1024 + 500)..(1024 + 1000)],
        metainfo_file.info().info_hash(),
        1,
        500,
        500,
        |_| (),
    );
    crate::send_block(
        &mut blocking_send,
        &files_bytes[(1024 + 1000)..(1024 + 1024)],
        metainfo_file.info().info_hash(),
        1,
        1000,
        24,
        |_| (),
    );

    // Send piece 2
    crate::send_block(
        &mut blocking_send,
        &files_bytes[(2048 + 0)..(2048 + 500)],
        metainfo_file.info().info_hash(),
        2,
        0,
        500,
        |_| (),
    );
    crate::send_block(
        &mut blocking_send,
        &files_bytes[(2048 + 500)..(2048 + 975)],
        metainfo_file.info().info_hash(),
        2,
        500,
        475,
        |_| (),
    );

    // Verify last two blocks are good
    let (piece_one_good, piece_two_good) = crate::core_loop_with_timeout(
        &mut core,
        500,
        ((false, false, 0), recv),
        |(piece_one_good, piece_two_good, messages_recvd), recv, msg| {
            let messages_recvd = messages_recvd + 1;

            // Map BlockProcessed to a None piece index so we don't update our state
            let (opt_piece_index, new_value) = match msg {
                ODiskMessage::FoundGoodPiece(_, index) => (Some(index), true),
                ODiskMessage::FoundBadPiece(_, index) => (Some(index), false),
                ODiskMessage::BlockProcessed(_) => (None, false),
                unexpected => panic!("Unexpected Message: {:?}", unexpected),
            };

            let (piece_one_good, piece_two_good) = match opt_piece_index {
                None => (piece_one_good, piece_two_good),
                Some(1) => (new_value, piece_two_good),
                Some(2) => (piece_one_good, new_value),
                Some(x) => panic!("Unexpected Index {:?}", x),
            };

            if messages_recvd == (5 + 2) {
                Loop::Break((piece_one_good, piece_two_good))
            } else {
                Loop::Continue(((piece_one_good, piece_two_good, messages_recvd), recv))
            }
        },
    );

    assert_eq!(true, piece_one_good);
    assert_eq!(true, piece_two_good);
}
