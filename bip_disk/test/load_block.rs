use {MultiFileDirectAccessor, InMemoryFileSystem};
use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage, BlockMetadata, Block, BlockMut};
use bip_metainfo::{MetainfoBuilder, PieceLength, MetainfoFile};
use bytes::BytesMut;
use tokio_core::reactor::{Core};
use futures::future::{Loop};
use futures::stream::Stream;
use futures::sink::Sink;

#[test]
fn positive_load_block() {
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

    let mut process_block = BytesMut::new();
    process_block.extend_from_slice(&data_b.0[1..(50 + 1)]);

    let mut load_block = BytesMut::with_capacity(50);
    load_block.extend_from_slice(&[0u8; 50]);

    let process_block = Block::new(BlockMetadata::new(metainfo_file.info_hash(), 1, 0, 50), process_block.freeze());
    let load_block    = BlockMut::new(BlockMetadata::new(metainfo_file.info_hash(), 1, 0, 50), load_block);

    let (send, recv) = disk_manager.split();
    let mut blocking_send = send.wait();
    blocking_send.send(IDiskMessage::AddTorrent(metainfo_file)).unwrap();

    let mut core = Core::new().unwrap();
    let (pblock, lblock) = ::core_loop_with_timeout(&mut core, 500, ((blocking_send, Some(process_block), Some(load_block)), recv),
        |(mut blocking_send, opt_pblock, opt_lblock), recv, msg| {
            match msg {
                ODiskMessage::TorrentAdded(_) => {
                    blocking_send.send(IDiskMessage::ProcessBlock(opt_pblock.unwrap())).unwrap();
                    Loop::Continue(((blocking_send, None, opt_lblock), recv))
                },
                ODiskMessage::BlockProcessed(block) => {
                    blocking_send.send(IDiskMessage::LoadBlock(opt_lblock.unwrap())).unwrap();
                    Loop::Continue(((blocking_send, Some(block), None), recv))
                },
                ODiskMessage::BlockLoaded(block) => Loop::Break((opt_pblock.unwrap(), block)),
                unexpected @ _ => panic!("Unexpected Message: {:?}", unexpected)
            }
        }
    );
    
    // Verify lblock contains our data
    assert_eq!(*pblock, *lblock);
}