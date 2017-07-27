use std::io;
use std::io::Write;

use nom::{IResult, be_u8};

/// Number of bytes that the extension protocol takes.
pub const NUM_EXTENSION_BYTES: usize = 8;

/// Enumeration of all extensions that can be activated.
pub enum Extension {
    /// Support for the extension protocol `http://www.bittorrent.org/beps/bep_0010.html`.
    ExtensionProtocol = 43
}

/// `Extensions` supported by either end of a handshake.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
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

    /// Add the given extension to the list of supported `Extensions`.
    pub fn add(&mut self, extension: Extension) {
        let active_bit = extension as usize;
        let byte_index = active_bit / 8;
        let bit_index = active_bit % 8;

        self.bytes[byte_index] |= 0x80 >> bit_index;
    }

    /// Remove the given extension from the list of supported `Extensions`.
    pub fn remove(&mut self, extension: Extension) {
        let active_bit = extension as usize;
        let byte_index = active_bit / 8;
        let bit_index = active_bit % 8;

        self.bytes[byte_index] &= !(0x80 >> bit_index);
    }

    /// Check if a given extension is activated.
    pub fn contains(&self, extension: Extension) -> bool {
        let active_bit = extension as usize;
        let byte_index = active_bit / 8;
        let bit_index = active_bit % 8;

        // Zero out all other bits, if result byte
        // is not equal to zero, we support it
        self.bytes[byte_index] & (0x80 >> bit_index) != 0
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

impl From<[u8; NUM_EXTENSION_BYTES]> for Extensions {
    fn from(bytes: [u8; NUM_EXTENSION_BYTES]) -> Extensions {
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

#[cfg(test)]
mod tests {
    use super::{Extensions, Extension};

    #[test]
    fn positive_add_extension_protocol() {
        let mut extensions = Extensions::new();
        extensions.add(Extension::ExtensionProtocol);

        let expected_extensions: Extensions = [0, 0, 0, 0, 0, 0x10, 0, 0].into();

        assert_eq!(expected_extensions, extensions);
        assert!(extensions.contains(Extension::ExtensionProtocol));
    }

    #[test]
    fn positive_remove_extension_protocol() {
        let mut extensions = Extensions::new();
        extensions.add(Extension::ExtensionProtocol);
        extensions.remove(Extension::ExtensionProtocol);

        let expected_extensions: Extensions = [0, 0, 0, 0, 0, 0, 0, 0].into();

        assert_eq!(expected_extensions, extensions);
        assert!(!extensions.contains(Extension::ExtensionProtocol));
    }
}