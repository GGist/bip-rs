use {MultiFileDirectAccessor, InMemoryFileSystem};
use bip_disk::{DiskManagerBuilder, IDiskMessage};
use bip_metainfo::{MetainfoBuilder, PieceLength, Metainfo};
use tokio_core::reactor::{Core};
use futures::future::{Future};
use futures::stream::Stream;
use futures::sink::Sink;
use futures::{future, AsyncSink};

#[test]
fn positive_disk_manager_send_backpressure() {
    // Create some "files" as random bytes
    let data_a = (::random_buffer(50), "/path/to/file/a".into());
    let data_b = (::random_buffer(2000), "/path/to/file/b".into());
    let data_c = (::random_buffer(0), "/path/to/file/c".into());

    // Create our accessor for our in memory files and create a torrent file for them
    let files_accessor = MultiFileDirectAccessor::new("/my/downloads/".into(),
        vec![data_a.clone(), data_b.clone(), data_c.clone()]);
    let metainfo_bytes = MetainfoBuilder::new()
        .set_piece_length(PieceLength::Custom(1024))
        .build(1, files_accessor, |_| ()).unwrap();
    let metainfo_file = Metainfo::from_bytes(metainfo_bytes).unwrap();
    let info_hash = metainfo_file.info().info_hash();

    // Spin up a disk manager and add our created torrent to it
    let filesystem = InMemoryFileSystem::new();
    let (m_send, m_recv) = DiskManagerBuilder::new()
        .with_sink_buffer_capacity(1)
        .build(filesystem.clone())
        .split();

    let mut core = Core::new().unwrap();
    
    // Add a torrent, so our receiver has a single torrent added message buffered
    let mut m_send = core.run(m_send.send(IDiskMessage::AddTorrent(metainfo_file))).unwrap();

    // Try to send a remove message (but it should fail)
    let (result, m_send) = core.run(future::lazy(|| future::ok::<_, ()>((m_send.start_send(IDiskMessage::RemoveTorrent(info_hash)), m_send)))).unwrap();
    match result {
        Ok(AsyncSink::NotReady(_)) => (),
        _                         => panic!("Unexpected Result From Backpressure Test")
    };

    // Receive from our stream to unblock the backpressure
    let m_recv = core.run(m_recv.into_future().map(|(_, recv)| recv).map_err(|_| ())).unwrap();
    
    // Try to send a remove message again which should go through
    let _ = core.run(m_send.send(IDiskMessage::RemoveTorrent(info_hash))).unwrap();

    // Receive confirmation (just so the pool doesnt panic because we ended before it could send the message back)
    let _ = core.run(m_recv.into_future().map(|(_, recv)| recv).map_err(|_| ())).unwrap();
}