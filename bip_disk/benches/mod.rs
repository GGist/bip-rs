#![feature(test)]

extern crate bip_disk;
extern crate bip_metainfo;
extern crate bytes;
extern crate futures;
extern crate test;
extern crate rand;

#[cfg(test)]
mod benches {
    use std::fs;

    use bip_disk::fs::NativeFileSystem;
    use bip_disk::fs_cache::FileHandleCache;
    use bip_disk::{DiskManagerBuilder, IDiskMessage, ODiskMessage, InfoHash, Block, BlockMetadata, FileSystem};
    use bip_metainfo::{DirectAccessor, MetainfoBuilder, Metainfo, PieceLength};
    use bytes::BytesMut;
    use futures::stream::{self, Stream};
    use futures::sink::{self, Sink};
    use rand::{self, Rng};
    use test::Bencher;

    /// Set to true if you are playing around with anything that could affect file
    /// sizes for an existing or new benchmarks. As a precaution, if the disk manager
    /// sees an existing file with a different size but same name as one of the files
    /// in the torrent, it wont touch it and a TorrentError will be generated.
    const WIPE_DATA_DIR: bool = false;

    // TODO: Benchmark multi file torrents!!!

    /// Generates a torrent with a single file of the given length.
    ///
    /// Returns both the torrent file, as well as the (random) data of the file.
    fn generate_single_file_torrent(piece_len: usize, file_len: usize) -> (Metainfo, Vec<u8>) {
        let mut rng = rand::weak_rng();
        
        let file_bytes: Vec<u8> = rng.gen_iter().take(file_len).collect();

        let metainfo_bytes = {
            let accessor = DirectAccessor::new("benchmark_file", &file_bytes[..]);

            MetainfoBuilder::new()
                .set_piece_length(PieceLength::Custom(piece_len))
                .build(1, accessor, |_| ())
                .unwrap()
        };
        let metainfo = Metainfo::from_bytes(&metainfo_bytes)
            .unwrap();

        (metainfo, file_bytes)
    }

    /// Adds the given metainfo file to the given sender, and waits for the added notification.
    fn add_metainfo_file<S, R>(metainfo: Metainfo, block_send: &mut sink::Wait<S>, block_recv: &mut stream::Wait<R>)    
        where S: Sink<SinkItem=IDiskMessage, SinkError=()>, R: Stream<Item=ODiskMessage, Error=()> {
        block_send.send(IDiskMessage::AddTorrent(metainfo)).unwrap();

        for res_message in block_recv {
            match res_message.unwrap() {
                ODiskMessage::TorrentAdded(_)      => { break; },
                ODiskMessage::FoundGoodPiece(_, _) => (),
                _                                  => panic!("Didn't Receive TorrentAdded")
            }
        }
    }

    /// Pushes the given bytes as piece blocks to the given sender, and blocks until all notifications
    /// of the blocks being processed have been received (does not check piece messages).
    fn process_blocks<S, R>(piece_length: usize, block_length: usize, hash: InfoHash, bytes: &[u8],
                            block_send: &mut sink::Wait<S>, block_recv: &mut stream::Wait<R>) 
        where S: Sink<SinkItem=IDiskMessage, SinkError=()>, R: Stream<Item=ODiskMessage, Error=()> {
        let mut blocks_sent = 0;

        for (piece_index, piece) in bytes.chunks(piece_length).enumerate() {
            for (block_index, block) in piece.chunks(block_length).enumerate() {
                let block_offset = block_index * block_length;
                let mut bytes = BytesMut::new();
                bytes.extend_from_slice(block);

                let block = Block::new(BlockMetadata::new(hash, piece_index as u64, block_offset as u64, block.len()), bytes.freeze());

                block_send.send(IDiskMessage::ProcessBlock(block)).unwrap();
                blocks_sent += 1;
            }
        }

        for res_message in block_recv {
            match res_message.unwrap() {
                ODiskMessage::BlockProcessed(_)    => { blocks_sent -= 1 },
                ODiskMessage::FoundGoodPiece(_, _) => (),
                ODiskMessage::FoundBadPiece(_, _)  => (),
                _                                  => panic!("Unexpected Message Received In process_blocks")
            }

            if blocks_sent == 0 {
                break;
            }
        }
    }

    /// Benchmarking method to setup a torrent file with the given attributes, and benchmark the block processing code.
    fn bench_process_file_with_fs<F>(b: &mut Bencher, piece_length: usize, block_length: usize, file_length: usize, fs: F)
        where F: FileSystem + Send + Sync + 'static {
        let (metainfo, bytes) = generate_single_file_torrent(piece_length, file_length);
        let info_hash = metainfo.info().info_hash();

        let disk_manager = DiskManagerBuilder::new()
            .with_sink_buffer_capacity(1_000_000)
            .with_stream_buffer_capacity(1_000_000)
            .build(fs);

        let (d_send, d_recv) = disk_manager.split();

        let mut block_d_send = d_send.wait();
        let mut block_d_recv = d_recv.wait();

        add_metainfo_file(metainfo, &mut block_d_send, &mut block_d_recv);

        b.iter(|| process_blocks(piece_length, block_length, info_hash, &bytes[..], &mut block_d_send, &mut block_d_recv))
    }

    #[bench]
    fn bench_native_fs_1_mb_pieces_128_kb_blocks(b: &mut Bencher) {
        let piece_length = 1 * 1024 * 1024;
        let block_length = 128 * 1024;
        let file_length = 2 * 1024 * 1024;
        let data_directory = "target/bench_data/bench_native_fs_1_mb_pieces_128_kb_blocks";

        if WIPE_DATA_DIR {
            let _ = fs::remove_dir_all(data_directory);
        }
        let filesystem = NativeFileSystem::with_directory(data_directory);

        bench_process_file_with_fs(b, piece_length, block_length, file_length, filesystem);
    }

    #[bench]
    fn bench_native_fs_1_mb_pieces_16_kb_blocks(b: &mut Bencher) {
        let piece_length = 1 * 1024 * 1024;
        let block_length = 16 * 1024;
        let file_length = 2 * 1024 * 1024;
        let data_directory = "target/bench_data/bench_native_fs_1_mb_pieces_16_kb_blocks";

        if WIPE_DATA_DIR {
            let _ = fs::remove_dir_all(data_directory);
        }
        let filesystem = NativeFileSystem::with_directory(data_directory);

        bench_process_file_with_fs(b, piece_length, block_length, file_length, filesystem);
    }

    #[bench]
    fn bench_native_fs_1_mb_pieces_2_kb_blocks(b: &mut Bencher) {
        let piece_length = 1 * 1024 * 1024;
        let block_length = 2 * 1024;
        let file_length = 2 * 1024 * 1024;
        let data_directory = "target/bench_data/bench_native_fs_1_mb_pieces_2_kb_blocks";

        if WIPE_DATA_DIR {
            let _ = fs::remove_dir_all(data_directory);
        }
        let filesystem = NativeFileSystem::with_directory(data_directory);

        bench_process_file_with_fs(b, piece_length, block_length, file_length, filesystem);
    }

    #[bench]
    fn bench_file_handle_cache_fs_1_mb_pieces_128_kb_blocks(b: &mut Bencher) {
        let piece_length = 1 * 1024 * 1024;
        let block_length = 128 * 1024;
        let file_length = 2 * 1024 * 1024;
        let data_directory = "target/bench_data/bench_native_fs_1_mb_pieces_128_kb_blocks";

        if WIPE_DATA_DIR {
            let _ = fs::remove_dir_all(data_directory);
        }
        let filesystem = FileHandleCache::new(NativeFileSystem::with_directory(data_directory), 1);

        bench_process_file_with_fs(b, piece_length, block_length, file_length, filesystem);
    }

    #[bench]
    fn bench_file_handle_cache_fs_1_mb_pieces_16_kb_blocks(b: &mut Bencher) {
        let piece_length = 1 * 1024 * 1024;
        let block_length = 16 * 1024;
        let file_length = 2 * 1024 * 1024;
        let data_directory = "target/bench_data/bench_native_fs_1_mb_pieces_16_kb_blocks";

        if WIPE_DATA_DIR {
            let _ = fs::remove_dir_all(data_directory);
        }
        let filesystem = FileHandleCache::new(NativeFileSystem::with_directory(data_directory), 1);

        bench_process_file_with_fs(b, piece_length, block_length, file_length, filesystem);
    }

    #[bench]
    fn bench_file_handle_cache_fs_1_mb_pieces_2_kb_blocks(b: &mut Bencher) {
        let piece_length = 1 * 1024 * 1024;
        let block_length = 2 * 1024;
        let file_length = 2 * 1024 * 1024;
        let data_directory = "target/bench_data/bench_native_fs_1_mb_pieces_2_kb_blocks";

        if WIPE_DATA_DIR {
            let _ = fs::remove_dir_all(data_directory);
        }
        let filesystem = FileHandleCache::new(NativeFileSystem::with_directory(data_directory), 1);

        bench_process_file_with_fs(b, piece_length, block_length, file_length, filesystem);
    }
}