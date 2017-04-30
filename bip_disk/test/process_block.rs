use std::time::Duration;

use {MultiFileDirectAccessor, InMemoryFileSystem};
use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage, FileSystem, BlockManager, BlockMetadata};
use bip_metainfo::{MetainfoBuilder, PieceLength, MetainfoFile};
use tokio_core::reactor::{Core, Timeout};
use futures::future::{self, Loop, Future};
use futures::stream::Stream;
use futures::sink::Sink;

#[test]
fn positive_process_block() {
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

    // Spin up a disk manager and add our created torrent to its
    let filesystem = InMemoryFileSystem::new();
    let disk_manager = DiskManagerBuilder::new()
        .build(filesystem.clone());

    // Spin up a block manager for allocating blocks
    let mut block_manager = BlockManager::new(2, 500).wait();

    let mut process_block = block_manager.next().unwrap().unwrap();
    // Start at the second byte of data_b and write 50 bytes
    process_block.set_metadata(BlockMetadata::new(metainfo_file.info_hash(), 1, 0, 50));
    // Copy over the actual data from data_b
    (&mut process_block[..50]).copy_from_slice(&data_b.0[1..(50 + 1)]);

    let (send, recv) = disk_manager.split();
    let mut blocking_send = send.wait();
    blocking_send.send(IDiskMessage::AddTorrent(metainfo_file)).unwrap();

    let mut core = Core::new().unwrap();
    let timeout = Timeout::new(Duration::from_millis(100), &core.handle()).unwrap()
        .then(|_| Err(()));
    core.run(
        future::loop_fn((blocking_send, recv, Some(process_block)), |(mut blocking_send, recv, opt_pblock)| {
            recv.into_future()
            .map(move |(opt_msg, recv)| {
                match opt_msg.unwrap() {
                    ODiskMessage::TorrentAdded(_) => {
                        blocking_send.send(IDiskMessage::ProcessBlock(opt_pblock.unwrap())).unwrap();
                        Loop::Continue((blocking_send, recv, None))
                    },
                    ODiskMessage::BlockProcessed(_)    => Loop::Break(()),
                    unexpected @ _ => panic!("Unexpected Message: {:?}", unexpected)
                }
            })
        })
        .map_err(|_| ())
        .select(timeout)
        .map(|(item, _)| item)
    ).unwrap_or_else(|_| panic!("Operation Failed Or Timed Out"));
    
    // Verify block was updated in data_b
    let mut received_file_b = filesystem.open_file(data_b.1).unwrap();
    assert_eq!(2000, filesystem.file_size(&received_file_b).unwrap());

    let mut recevied_file_b_data = vec![0u8; 2000];
    assert_eq!(2000, filesystem.read_file(&mut received_file_b, 0, &mut recevied_file_b_data).unwrap());

    let mut expected_file_b_data = vec![0u8; 2000];
    (&mut expected_file_b_data[1..(1 + 50)]).copy_from_slice(&data_b.0[1..(50 + 1)]);
    assert_eq!(expected_file_b_data, recevied_file_b_data);
}