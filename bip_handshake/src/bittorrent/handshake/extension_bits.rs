use std::io;
use std::io::Write;

use nom::{IResult, be_u8};

/// Number of bytes that the extension protocol takes.
const NUM_EXTENSION_BYTES: usize = 8;

/// Contains handshake extension bits which specify which extended
/// bittorrent peer wire protocol messages, the client supports.
pub struct ExtensionBits {
    bytes: [u8; NUM_EXTENSION_BYTES]
}

impl ExtensionBits {
    /// Create a new `ExtensionBits` with zero extensions.
    pub fn new() -> ExtensionBits {
        ExtensionBits::with_bytes([0u8; NUM_EXTENSION_BYTES])
    }

    /// Create a new `ExtensionBits` by parsing the given bytes.
    pub fn from_bytes(bytes: &[u8]) -> IResult<&[u8], ExtensionBits> {
        parse_extension_bits(bytes)
    }

    /// Write the `ExtensionBits` to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        writer.write_all(&self.bytes[..])
    }

    /// Create a new `ExtensionBits` using the given bytes directly.
    fn with_bytes(bytes: [u8; NUM_EXTENSION_BYTES]) -> ExtensionBits {
        ExtensionBits{ bytes: bytes }
    }
}

/// Parse the given bytes for extension bits.
fn parse_extension_bits(bytes: &[u8]) -> IResult<&[u8], ExtensionBits> {
    do_parse!(bytes,
        bytes: count_fixed!(u8, be_u8, NUM_EXTENSION_BYTES) >>
        (ExtensionBits::with_bytes(bytes))
    )
}