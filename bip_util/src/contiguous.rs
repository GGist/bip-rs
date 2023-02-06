use std::cmp;

/// Trait for metadata, reading, and writing to a contiguous buffer that doesn't
/// re allocate.
pub trait ContiguousBuffer<T> {
    /// Total capacity of the underlying buffer.
    fn capacity(&self) -> usize;

    /// Total length, or used capacity, of the underlying buffer.
    fn length(&self) -> usize;

    /// Clear the used capacity of the underlying buffer without re allocating.
    fn clear(&mut self);

    /// Write to the unused capacity of the buffer.
    ///
    /// Panic will occur if more data is given than capacity is available for.
    fn write(&mut self, data: &[T]);

    /// Read the used capacity of the buffer.
    ///
    /// Implementations will never pass in an empty slice.
    fn read<F>(&self, receive: F)
    where
        F: FnMut(&[T]);
}

impl<T> ContiguousBuffer<T> for Vec<T>
where
    T: Clone,
{
    fn capacity(&self) -> usize {
        self.capacity()
    }

    fn length(&self) -> usize {
        self.len()
    }

    fn clear(&mut self) {
        self.truncate(0);
    }

    fn write(&mut self, data: &[T]) {
        let available_space = self.capacity() - self.length();

        if data.len() > available_space {
            panic!("bip_util: ContiguousBuffer::write Detected Write That Overflows Vec");
        } else {
            self.extend_from_slice(data);
        }
    }

    fn read<F>(&self, mut receive: F)
    where
        F: FnMut(&[T]),
    {
        if self.length() > 0 {
            receive(&self[..]);
        }
    }
}

//----------------------------------------------------------------------------//

/// Struct for providing a ContiguousBuffer abstraction over many contiguous
/// buffers.
pub struct ContiguousBuffers<T> {
    buffers: Vec<T>,
}

impl<T> ContiguousBuffers<T> {
    /// Create a new empty ContiguousBuffers struct.
    pub fn new() -> ContiguousBuffers<T> {
        ContiguousBuffers {
            buffers: Vec::new(),
        }
    }

    /// Create a new ContiguousBuffers struct with an initial element.
    pub fn with_buffer(buffer: T) -> ContiguousBuffers<T> {
        ContiguousBuffers {
            buffers: vec![buffer],
        }
    }

    /// Pack a value T at the end of the contiguous buffers.
    pub fn pack(&mut self, mut buffer: ContiguousBuffers<T>) {
        self.buffers.append(&mut buffer.buffers);
    }

    /// Unpack all values T and pass them to the given closure.
    pub fn unpack<F>(self, mut accept: F)
    where
        F: FnMut(ContiguousBuffers<T>),
    {
        for buffer in self.buffers {
            accept(ContiguousBuffers::with_buffer(buffer));
        }
    }
}

impl<T, I> ContiguousBuffer<I> for ContiguousBuffers<T>
where
    T: ContiguousBuffer<I>,
    I: Clone,
{
    fn capacity(&self) -> usize {
        self.buffers.iter().map(|buffer| buffer.capacity()).sum()
    }

    fn length(&self) -> usize {
        self.buffers.iter().map(|buffer| buffer.length()).sum()
    }

    fn clear(&mut self) {
        for buffer in self.buffers.iter_mut() {
            buffer.clear();
        }
    }

    fn write(&mut self, data: &[I]) {
        let mut bytes_written = 0;

        for buffer in self.buffers.iter_mut() {
            if bytes_written == data.len() {
                break;
            }
            let available_capacity = buffer.capacity() - buffer.length();
            let amount_to_write = cmp::min(available_capacity, data.len() - bytes_written);

            let (start, end) = (bytes_written, bytes_written + amount_to_write);

            buffer.write(&data[start..end]);
            bytes_written += amount_to_write;
        }

        // If we exhausted all of our buffers but we didn't write all the data yet
        if bytes_written != data.len() {
            panic!(
                "bip_util: ContiguousBuffer::write Detected Write That Overflows ContiguousBuffers"
            );
        }
    }

    fn read<F>(&self, mut receive: F)
    where
        F: FnMut(&[I]),
    {
        for buffer in self.buffers.iter() {
            buffer.read(&mut receive);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ContiguousBuffer, ContiguousBuffers};

    #[test]
    #[should_panic]
    fn positive_write_no_buffers() {
        let mut buffers: ContiguousBuffers<Vec<u8>> = ContiguousBuffers::new();

        buffers.write(b"1");
    }

    #[test]
    fn positive_write_single_buffer_partially_filled() {
        let mut buffers = ContiguousBuffers::with_buffer(Vec::with_capacity(5));

        buffers.write(b"1111");

        assert_eq!(5, buffers.capacity());
        assert_eq!(4, buffers.length());
    }

    #[test]
    fn positive_write_single_buffer_completely_filled() {
        let mut buffers = ContiguousBuffers::with_buffer(Vec::with_capacity(5));

        buffers.write(b"11111");

        assert_eq!(5, buffers.capacity());
        assert_eq!(5, buffers.length());
    }

    #[test]
    fn positive_write_mutliple_buffers_partially_filled() {
        let mut buffers = ContiguousBuffers::new();
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(1)));
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(5)));

        buffers.write(b"11");

        assert_eq!(6, buffers.capacity());
        assert_eq!(2, buffers.length());
    }

    #[test]
    fn positive_write_multiple_buffers_completely_filled() {
        let mut buffers = ContiguousBuffers::new();
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(1)));
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(5)));

        buffers.write(b"111111");

        assert_eq!(6, buffers.capacity());
        assert_eq!(6, buffers.length());
    }

    #[test]
    fn positive_read_no_buffers() {
        let buffers: ContiguousBuffers<Vec<u8>> = ContiguousBuffers::new();

        let mut times_called = 0;
        buffers.read(|_| {
            times_called += 1;
        });

        assert_eq!(0, times_called);
    }

    #[test]
    fn positive_read_single_buffer_empty() {
        let buffers: ContiguousBuffers<Vec<u8>> =
            ContiguousBuffers::with_buffer(Vec::with_capacity(10));

        let mut times_called = 0;
        buffers.read(|_| {
            times_called += 1;
        });

        assert_eq!(0, times_called);
    }

    #[test]
    fn positive_read_multiple_buffers_empty() {
        let mut buffers: ContiguousBuffers<Vec<u8>> = ContiguousBuffers::new();
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(1)));
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(1)));

        let mut times_called = 0;
        buffers.read(|_| {
            times_called += 1;
        });

        assert_eq!(0, times_called);
    }

    #[test]
    fn positive_read_single_buffer_partially_filled() {
        let mut buffers = ContiguousBuffers::with_buffer(Vec::with_capacity(5));

        buffers.write(b"1111");

        let mut times_called = 0;
        let mut read_buffer = Vec::new();
        buffers.read(|buffer| {
            times_called += 1;
            read_buffer.extend_from_slice(buffer);
        });

        assert_eq!(1, times_called);
        assert_eq!(&b"1111"[..], &read_buffer[..]);
    }

    #[test]
    fn positive_read_single_buffer_completely_filled() {
        let mut buffers = ContiguousBuffers::with_buffer(Vec::with_capacity(5));

        buffers.write(b"11111");

        let mut times_called = 0;
        let mut read_buffer = Vec::new();
        buffers.read(|buffer| {
            times_called += 1;
            read_buffer.extend_from_slice(buffer);
        });

        assert_eq!(1, times_called);
        assert_eq!(&b"11111"[..], &read_buffer[..]);
    }

    #[test]
    fn positive_read_mutliple_buffers_partially_filled() {
        let mut buffers = ContiguousBuffers::new();
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(1)));
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(5)));

        buffers.write(b"1111");

        let mut times_called = 0;
        let mut read_buffer = Vec::new();
        buffers.read(|buffer| {
            times_called += 1;
            read_buffer.extend_from_slice(buffer);
        });

        assert_eq!(2, times_called);
        assert_eq!(&b"1111"[..], &read_buffer[..]);
    }

    #[test]
    fn positive_read_multiple_buffers_completely_filled() {
        let mut buffers = ContiguousBuffers::new();
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(1)));
        buffers.pack(ContiguousBuffers::with_buffer(Vec::with_capacity(5)));

        buffers.write(b"111111");

        let mut times_called = 0;
        let mut read_buffer = Vec::new();
        buffers.read(|buffer| {
            times_called += 1;
            read_buffer.extend_from_slice(buffer);
        });

        assert_eq!(2, times_called);
        assert_eq!(&b"111111"[..], &read_buffer[..]);
    }
}
