use std::time::Duration;

use {MultiFileDirectAccessor, InMemoryFileSystem};
use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage, BlockManager, BlockMetadata};
use bip_metainfo::{MetainfoBuilder, PieceLength, MetainfoFile};
use tokio_core::reactor::{Core, Timeout};
use futures::future::{self, Loop, Future};
use futures::stream::Stream;
use futures::sink::Sink;

#[test]
fn positive_remove_torrent() {
    // Create some "files" as random bytes
    let data_a = (::random_buffer(50), "/path/to/file/a".into());
    let data_b = (::random_buffer(2000), "/path/to/file/b".into());
    let data_c = (::random_buffer(0), "/path/to/file/c".into());

    // Create our accessor for our in memory files and create a torrent file for them
    let files_accessor = MultiFileDirectAccessor::new("/my/downloads/".into(),
        vec![data_a.clone(), data_b.clone(), data_c.clone()]);
    let metainfo_bytes = MetainfoBuilder::new()
        .set_piece_length(PieceLength::Custom(1024))
        .build_as_bytes(1, files_accessor, |_| ()).unwrap();
    let metainfo_file = MetainfoFile::from_bytes(metainfo_bytes).unwrap();
    let info_hash = metainfo_file.info_hash();

    // Spin up a disk manager and add our created torrent to it
    let filesystem = InMemoryFileSystem::new();
    let disk_manager = DiskManagerBuilder::new()
        .build(filesystem.clone());

    let (send, recv) = disk_manager.split();
    let mut blocking_send = send.wait();
    blocking_send.send(IDiskMessage::AddTorrent(metainfo_file)).unwrap();

    // Verify that zero pieces are marked as good
    let mut core = Core::new().unwrap();
    let timeout = Timeout::new(Duration::from_millis(100), &core.handle()).unwrap()
        .then(|_| Err(()));
    let (good_pieces, mut blocking_send, recv) = core.run(
        future::loop_fn((recv, blocking_send, 0), |(recv, mut blocking_send, good)| {
            recv.into_future()
            .map(move |(opt_msg, recv)| {
                match opt_msg.unwrap() {
                    ODiskMessage::TorrentAdded(_)      => {
                        blocking_send.send(IDiskMessage::RemoveTorrent(info_hash)).unwrap();
                        Loop::Continue((recv, blocking_send, good))
                    },
                    ODiskMessage::TorrentRemoved(_)    => Loop::Break((good, blocking_send, recv)),
                    ODiskMessage::FoundGoodPiece(_, _) => Loop::Continue((recv, blocking_send, good + 1)),
                    unexpected @ _                     => panic!("Unexpected Message: {:?}", unexpected)
                }
            })
        })
        .map_err(|_| ())
        .select(timeout)
        .map(|(item, _)| item)
    ).unwrap_or_else(|_| panic!("Add Torrent Operation Failed Or Timed Out"));

    assert_eq!(0, good_pieces);

    // Try to process a block for our removed torrent
    let mut block_manager = BlockManager::new(1, 50).wait();

    let mut process_block = block_manager.next().unwrap().unwrap();
    // Start at the first byte of data_a and write 50 bytes
    process_block.set_metadata(BlockMetadata::new(info_hash, 0, 0, 50));
    // Copy over the actual data from data_a
    (&mut process_block[..50]).copy_from_slice(&data_a.0[0..50]);

    blocking_send.send(IDiskMessage::ProcessBlock(process_block)).unwrap();

    let timeout = Timeout::new(Duration::from_millis(100), &core.handle()).unwrap()
        .then(|_| Err(()));
    core.run(
        future::loop_fn(recv, |recv| {
            recv.into_future()
            .map(move |(opt_msg, _)| {
                match opt_msg.unwrap() {
                    ODiskMessage::BlockError(_, _) => Loop::Break(()),
                    unexpected @ _                 => panic!("Unexpected Message: {:?}", unexpected)
                }
            })
        })
        .map_err(|_| ())
        .select(timeout)
        .map(|(item, _)| item)
    ).unwrap_or_else(|_| panic!("Process Block Operation Failed Or Timed Out"));
}