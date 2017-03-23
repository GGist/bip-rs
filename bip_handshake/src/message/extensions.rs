use std::io;
use std::io::Write;

use nom::{IResult, be_u8};

/// Number of bytes that the extension protocol takes.
pub const NUM_EXTENSION_BYTES: usize = 8;

/// Extensions supported by either end of a handshake.
#[derive(Copy, Clone)]
pub struct Extensions {
    bytes: [u8; NUM_EXTENSION_BYTES]
}

impl Extensions {
    /// Create a new `Extensions` with zero extensions.
    pub fn new() -> Extensions {
        Extensions::with_bytes([0u8; NUM_EXTENSION_BYTES])
    }

    /// Create a new `Extensions` by parsing the given bytes.
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], Extensions> {
        parse_extension_bits(bytes)
    }

    /// Write the `Extensions` to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        writer.write_all(&self.bytes[..])
    }

    /// Create a union of the two extensions.
    ///
    /// This is useful for getting the extensions that both clients support.
    pub fn union(&self, ext: &Extensions) -> Extensions {
        let mut result_ext = Extensions::new();

        for index in 0..NUM_EXTENSION_BYTES {
            result_ext.bytes[index] = self.bytes[index] & ext.bytes[index];
        }

        result_ext
    }

    /// Create a new `Extensions` using the given bytes directly.
    fn with_bytes(bytes: [u8; NUM_EXTENSION_BYTES]) -> Extensions {
        Extensions{ bytes: bytes }
    }
}

/// Parse the given bytes for extension bits.
fn parse_extension_bits(bytes: &[u8]) -> IResult<&[u8], Extensions> {
    do_parse!(bytes,
        bytes: count_fixed!(u8, be_u8, NUM_EXTENSION_BYTES) >>
        (Extensions::with_bytes(bytes))
    )
}