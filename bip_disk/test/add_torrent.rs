use {MultiFileDirectAccessor, InMemoryFileSystem};
use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage, FileSystem};
use bip_metainfo::{MetainfoBuilder, PieceLength, MetainfoFile};
use tokio_core::reactor::{Core};
use futures::future::{Loop, Future};
use futures::stream::Stream;
use futures::sink::Sink;

#[test]
fn positive_add_torrent() {
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

    // Spin up a disk manager and add our created torrent to it
    let filesystem = InMemoryFileSystem::new();
    let disk_manager = DiskManagerBuilder::new()
        .build(filesystem.clone());

    let (send, recv) = disk_manager.split();
    send.send(IDiskMessage::AddTorrent(metainfo_file)).wait().unwrap();

    // Verify that zero pieces are marked as good
    let mut core = Core::new().unwrap();

    // Run a core loop until we get the TorrentAdded message
    let good_pieces = ::core_loop_with_timeout(&mut core, 100, (0, recv), |good_pieces, recv, msg| {
        match msg {
            ODiskMessage::TorrentAdded(_)      => Loop::Break(good_pieces),
            ODiskMessage::FoundGoodPiece(_, _) => Loop::Continue((good_pieces + 1, recv)),
            unexpected @ _                     => panic!("Unexpected Message: {:?}", unexpected)
        }
    });

    assert_eq!(0, good_pieces);

    // Verify file a in file system
    let mut received_file_a = filesystem.open_file(data_a.1).unwrap();
    assert_eq!(50, filesystem.file_size(&received_file_a).unwrap());

    let mut received_buffer_a = vec![0u8; 50];
    assert_eq!(50, filesystem.read_file(&mut received_file_a, 0, &mut received_buffer_a[..]).unwrap());
    assert_eq!(vec![0u8; 50], received_buffer_a);

    // Verify file b in file system
    let mut received_file_b = filesystem.open_file(data_b.1).unwrap();
    assert_eq!(2000, filesystem.file_size(&received_file_b).unwrap());

    let mut received_buffer_b = vec![0u8; 2000];
    assert_eq!(2000, filesystem.read_file(&mut received_file_b, 0, &mut received_buffer_b[..]).unwrap());
    assert_eq!(vec![0u8; 2000], received_buffer_b);

    // Verify file c in file system
    let mut received_file_c = filesystem.open_file(data_c.1).unwrap();
    assert_eq!(0, filesystem.file_size(&received_file_c).unwrap());

    let mut received_buffer_c = vec![0u8; 0];
    assert_eq!(0, filesystem.read_file(&mut received_file_c, 0, &mut received_buffer_c[..]).unwrap());
    assert_eq!(vec![0u8; 0], received_buffer_c);
}