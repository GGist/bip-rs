use crate::{MultiFileDirectAccessor, InMemoryFileSystem};
use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage, BlockMetadata, Block};
use bip_metainfo::{MetainfoBuilder, PieceLength, Metainfo};
use bytes::BytesMut;
use tokio_core::reactor::{Core};
use futures::future::{Loop};
use futures::stream::Stream;
use futures::sink::Sink;

#[test]
fn positive_remove_torrent() {
    // Create some "files" as random bytes
    let data_a = (crate::random_buffer(50), "/path/to/file/a".into());
    let data_b = (crate::random_buffer(2000), "/path/to/file/b".into());
    let data_c = (crate::random_buffer(0), "/path/to/file/c".into());

    // Create our accessor for our in memory files and create a torrent file for them
    let files_accessor = MultiFileDirectAccessor::new("/my/downloads/".into(),
        vec![data_a.clone(), data_b, data_c]);
    let metainfo_bytes = MetainfoBuilder::new()
        .set_piece_length(PieceLength::Custom(1024))
        .build(1, files_accessor, |_| ()).unwrap();
    let metainfo_file = Metainfo::from_bytes(metainfo_bytes).unwrap();
    let info_hash = metainfo_file.info().info_hash();

    // Spin up a disk manager and add our created torrent to it
    let filesystem = InMemoryFileSystem::new();
    let disk_manager = DiskManagerBuilder::new()
        .build(filesystem);

    let (send, recv) = disk_manager.split();
    let mut blocking_send = send.wait();
    blocking_send.send(IDiskMessage::AddTorrent(metainfo_file)).unwrap();

    // Verify that zero pieces are marked as good
    let mut core = Core::new().unwrap();

    let (mut blocking_send, good_pieces, recv) = crate::core_loop_with_timeout(&mut core, 500, ((blocking_send, 0), recv),
        |(mut blocking_send, good_pieces), recv, msg| {
            match msg {
                ODiskMessage::TorrentAdded(_)      => {
                    blocking_send.send(IDiskMessage::RemoveTorrent(info_hash)).unwrap();
                    Loop::Continue(((blocking_send, good_pieces), recv))
                },
                ODiskMessage::TorrentRemoved(_)    => Loop::Break((blocking_send, good_pieces, recv)),
                ODiskMessage::FoundGoodPiece(_, _) => Loop::Continue(((blocking_send, good_pieces + 1), recv)),
                unexpected                     => panic!("Unexpected Message: {:?}", unexpected)
            }
    });

    assert_eq!(0, good_pieces);

    let mut process_bytes = BytesMut::new();
    process_bytes.extend_from_slice(&data_a.0[0..50]);

    let process_block = Block::new(BlockMetadata::new(info_hash, 0, 0, 50), process_bytes.freeze());

    blocking_send.send(IDiskMessage::ProcessBlock(process_block)).unwrap();

    crate::core_loop_with_timeout(&mut core, 500, ((), recv),
        |_, _, msg| {
            match msg {
                ODiskMessage::ProcessBlockError(_, _) => Loop::Break(()),
                unexpected                            => panic!("Unexpected Message: {:?}", unexpected)
            }
    });
}