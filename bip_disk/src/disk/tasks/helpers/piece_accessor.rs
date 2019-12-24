use std::cmp;
use std::io;

use crate::disk::fs::FileSystem;
use crate::disk::tasks::helpers;
use crate::memory::block::BlockMetadata;

use bip_metainfo::Info;

pub struct PieceAccessor<'a, F> {
    fs: F,
    info_dict: &'a Info,
}

impl<'a, F> PieceAccessor<'a, F>
where
    F: FileSystem,
{
    pub fn new(fs: F, info_dict: &'a Info) -> PieceAccessor<'a, F> {
        PieceAccessor { fs, info_dict }
    }

    pub fn read_piece(&self, piece_buffer: &mut [u8], message: &BlockMetadata) -> io::Result<()> {
        self.run_with_file_regions(message, |mut file, offset, begin, end| {
            let bytes_read = self.fs.read_file(&mut file, offset, &mut piece_buffer[begin..end])?;
            assert_eq!(bytes_read, end - begin);

            Ok(())
        })
    }

    pub fn write_piece(&self, piece_buffer: &[u8], message: &BlockMetadata) -> io::Result<()> {
        self.run_with_file_regions(message, |mut file, offset, begin, end| {
            let bytes_written = self.fs.write_file(&mut file, offset, &piece_buffer[begin..end])?;
            assert_eq!(bytes_written, end - begin);

            Ok(())
        })
    }

    /// Run the given closure with the file, the file offset, and the read/write buffer stard (inclusive) and end (exclusive) indices.
    /// TODO: We do not detect when/if the file size changes after the initial file size check, so the returned number of
    fn run_with_file_regions<C>(&self, message: &BlockMetadata, mut callback: C) -> io::Result<()>
    where
        C: FnMut(F::File, u64, usize, usize) -> io::Result<()>,
    {
        let piece_length = self.info_dict.piece_length() as u64;

        let mut total_bytes_to_skip = (message.piece_index() * piece_length) + message.block_offset();
        let mut total_bytes_accessed = 0;
        let total_block_length = message.block_length() as u64;

        for file in self.info_dict.files() {
            let total_file_size = file.length() as u64;

            let mut bytes_to_access = total_file_size;
            let min_bytes_to_skip = cmp::min(total_bytes_to_skip, bytes_to_access);

            total_bytes_to_skip -= min_bytes_to_skip;
            bytes_to_access -= min_bytes_to_skip;

            if bytes_to_access > 0 && total_bytes_accessed < total_block_length {
                let file_path = helpers::build_path(self.info_dict.directory(), file);
                let fs_file = self.fs.open_file(file_path)?;

                let total_max_bytes_to_access = total_block_length - total_bytes_accessed;
                let actual_bytes_to_access = cmp::min(total_max_bytes_to_access, bytes_to_access);
                let offset = total_file_size - bytes_to_access;

                let (begin, end) = (total_bytes_accessed as usize, (total_bytes_accessed + actual_bytes_to_access) as usize);
                callback(fs_file, offset, begin, end)?;
                total_bytes_accessed += actual_bytes_to_access;
            }
        }

        Ok(())
    }
}
