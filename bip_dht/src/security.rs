// TODO: Remove this when we actually use the security module.
#![allow(unused)]

use std::net::Ipv4Addr;

use bip_util::bt::{self, NodeId};
use bip_util::convert;
use crc::crc32;
use rand;

const IPV4_MASK: u32 = 0x030F_3FFF;
const IPV6_MASK: u64 = 0x0103_070F_1F3F_7FFF;

const CRC32C_ARG_SLICE_SIZE: usize = 8;

// TODO: Add IPv6 support, only when proper unit tests have been constructed

// ----------------------------------------------------------------------------//

/// Generates an ipv4 address compliant node id.
pub fn generate_compliant_id_ipv4(addr: Ipv4Addr) -> NodeId {
    let masked_ipv4_be = mask_ipv4_be(addr);
    let rand = rand::random::<u8>();

    NodeId::from(generate_compliant_id(masked_ipv4_be as u64, 4, rand))
}

/// Generates an ip address compliant node id.
fn generate_compliant_id(masked_ip_be: u64, num_octets: usize, rand: u8) -> [u8; bt::NODE_ID_LEN] {
    let r = rand & 0x07;

    let mut masked_ip_bytes = convert::eight_bytes_to_array(masked_ip_be);
    let starting_byte = masked_ip_bytes.len() - num_octets;
    masked_ip_bytes[starting_byte] |= r << 5;

    let crc32c_result = crc32::checksum_castagnoli(&masked_ip_bytes[starting_byte..]);

    let mut node_id = [0u8; bt::NODE_ID_LEN];
    node_id[0] = (crc32c_result >> 24) as u8;
    node_id[1] = (crc32c_result >> 16) as u8;
    node_id[2] = (((crc32c_result >> 8) & 0xF8) as u8) | (rand::random::<u8>() & 0x07);
    for byte in node_id[3..19].iter_mut() {
        *byte = rand::random::<u8>();
    }
    node_id[19] = rand;

    node_id
}

// ----------------------------------------------------------------------------//

/// Compares the given ipv4 address against the given node id to see if the node id is valid.
pub fn is_compliant_ipv4_addr(addr: Ipv4Addr, id: NodeId) -> bool {
    if is_security_compliant_ipv4_exempt(addr) {
        return true;
    }
    let masked_ip_be = mask_ipv4_be(addr) as u64;

    is_compliant_addr(masked_ip_be, 4, id)
}

/// Checks to see if the given ipv4 address is exempt from a security check.
fn is_security_compliant_ipv4_exempt(addr: Ipv4Addr) -> bool {
    // TODO: Since we are not using this module yet, we dont have to use the ip feature gate which is not stable yet.

    false
    // addr.is_loopback() || addr.is_private() || addr.is_link_local()
}

/// Compares the given masked ip (v4 or v6) against the given node id to see if the node if is valid.
///
/// Also, LOL, the spec is so confusing when it comes to this and mixes variable naming multiple times.
/// If you understand how to generate a compliant id, essentially assume the id is legit, take the rand
/// variable which should be the last byte of the node id, and then basically do what we would do when
/// generating an id (aside from generating filler random numbers which would be wasteful here).
fn is_compliant_addr(masked_ip_be: u64, num_octets: usize, id: NodeId) -> bool {
    if num_octets > CRC32C_ARG_SLICE_SIZE {
        panic!("error in dht::security::is_compliant_addr(), num_octets is greater than buffer \
                size")
    }
    let id_bytes = Into::<[u8; bt::NODE_ID_LEN]>::into(id);

    let rand = id_bytes[19];
    let r = rand & 0x07;

    let ip_bits_used = num_octets * 8;
    let rand_masked_ip = masked_ip_be | ((r as u64) << (ip_bits_used - 3));

    // Move the rand_masked_ip bytes over to an array for running through crc32c
    let mut rand_masked_ip_bytes = convert::eight_bytes_to_array(rand_masked_ip);
    let starting_byte = rand_masked_ip_bytes.len() - num_octets;

    // Official spec says to store the rand_masked_ip in a 64 bit integer (8 byte array) and hash
    // the result, however, the rasterbar spec says to store them in a 32 bit integer (4 byte
    // array). We are performing the latter as it seems to give us correct results for the
    // different test vectors. We will however store 8 octet addresses in an 8 byte array.

    // TODO: Not sure if this checksum uses a constant internally that depends on endiannes of computer
    // (this sentence is most likely stupid in more than one way).
    let crc32c_result = crc32::checksum_castagnoli(&rand_masked_ip_bytes[4..]);

    is_compliant_id(crc32c_result, id_bytes)
}

/// Compares the result from the crc32c function against the first 21 bits of the node id.
///
/// We dont have to check the last byte of the node id since we used that byte to generate
/// the crc32c_result.
fn is_compliant_id(crc32c_result: u32, id_bytes: [u8; bt::NODE_ID_LEN]) -> bool {
    let mut is_compliant = true;
    is_compliant = is_compliant && (id_bytes[0] == ((crc32c_result >> 24) as u8));
    is_compliant = is_compliant && (id_bytes[1] == ((crc32c_result >> 16) as u8));

    let mid_id_bits = (id_bytes[2] >> 0) & 0xF8;
    let mid_hash_bits = ((crc32c_result >> 8) as u8) & 0xF8;
    is_compliant = is_compliant && mid_id_bits == mid_hash_bits;

    is_compliant
}

// ----------------------------------------------------------------------------//

/// Perform the initial mask of an ipv4 address.
fn mask_ipv4_be(addr: Ipv4Addr) -> u32 {
    let ip_be = Into::<u32>::into(addr);

    ip_be & IPV4_MASK
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    const IPV4_ONE: (u8, u8, u8, u8) = (124, 31, 75, 21);
    const IPV4_ONE_RAND: u8 = 1;
    const IPV4_ONE_BITS: (u8, u8, u8) = (0x5F, 0xBF, 0xB8);

    const IPV4_TWO: (u8, u8, u8, u8) = (21, 75, 31, 124);
    const IPV4_TWO_RAND: u8 = 86;
    const IPV4_TWO_BITS: (u8, u8, u8) = (0x5A, 0x3C, 0xE8);

    const IPV4_THREE: (u8, u8, u8, u8) = (65, 23, 51, 170);
    const IPV4_THREE_RAND: u8 = 22;
    const IPV4_THREE_BITS: (u8, u8, u8) = (0xA5, 0xD4, 0x30);

    const IPV4_FOUR: (u8, u8, u8, u8) = (84, 124, 73, 14);
    const IPV4_FOUR_RAND: u8 = 65;
    const IPV4_FOUR_BITS: (u8, u8, u8) = (0x1B, 0x03, 0x20);

    const IPV4_FIVE: (u8, u8, u8, u8) = (43, 213, 53, 83);
    const IPV4_FIVE_RAND: u8 = 90;
    const IPV4_FIVE_BITS: (u8, u8, u8) = (0xE5, 0x6F, 0x68);

    #[test]
    fn positive_generate_compliant_ipv4_test_one() {
        let ipv4_addr = Ipv4Addr::new(IPV4_ONE.0, IPV4_ONE.1, IPV4_ONE.2, IPV4_ONE.3);
        let masked_ip_be = super::mask_ipv4_be(ipv4_addr) as u64;

        let node_id = super::generate_compliant_id(masked_ip_be, 4, IPV4_ONE_RAND);

        assert_eq!(node_id[0], IPV4_ONE_BITS.0);
        assert_eq!(node_id[1], IPV4_ONE_BITS.1);

        assert_eq!(node_id[2] & 0xF8, IPV4_ONE_BITS.2);

        assert_eq!(node_id[19], IPV4_ONE_RAND);
    }

    #[test]
    fn positive_generate_compliant_ipv4_test_two() {
        let ipv4_addr = Ipv4Addr::new(IPV4_TWO.0, IPV4_TWO.1, IPV4_TWO.2, IPV4_TWO.3);
        let masked_ip_be = super::mask_ipv4_be(ipv4_addr) as u64;

        let node_id = super::generate_compliant_id(masked_ip_be, 4, IPV4_TWO_RAND);

        assert_eq!(node_id[0], IPV4_TWO_BITS.0);
        assert_eq!(node_id[1], IPV4_TWO_BITS.1);

        assert_eq!(node_id[2] & 0xF8, IPV4_TWO_BITS.2);

        assert_eq!(node_id[19], IPV4_TWO_RAND);
    }

    #[test]
    fn positive_generate_compliant_ipv4_test_three() {
        let ipv4_addr = Ipv4Addr::new(IPV4_THREE.0, IPV4_THREE.1, IPV4_THREE.2, IPV4_THREE.3);
        let masked_ip_be = super::mask_ipv4_be(ipv4_addr) as u64;

        let node_id = super::generate_compliant_id(masked_ip_be, 4, IPV4_THREE_RAND);

        assert_eq!(node_id[0], IPV4_THREE_BITS.0);
        assert_eq!(node_id[1], IPV4_THREE_BITS.1);

        assert_eq!(node_id[2] & 0xF8, IPV4_THREE_BITS.2);

        assert_eq!(node_id[19], IPV4_THREE_RAND);
    }

    #[test]
    fn positive_generate_compliant_ipv4_test_four() {
        let ipv4_addr = Ipv4Addr::new(IPV4_FOUR.0, IPV4_FOUR.1, IPV4_FOUR.2, IPV4_FOUR.3);
        let masked_ip_be = super::mask_ipv4_be(ipv4_addr) as u64;

        let node_id = super::generate_compliant_id(masked_ip_be, 4, IPV4_FOUR_RAND);

        assert_eq!(node_id[0], IPV4_FOUR_BITS.0);
        assert_eq!(node_id[1], IPV4_FOUR_BITS.1);

        assert_eq!(node_id[2] & 0xF8, IPV4_FOUR_BITS.2);

        assert_eq!(node_id[19], IPV4_FOUR_RAND);
    }

    #[test]
    fn positive_generate_compliant_ipv4_test_five() {
        let ipv4_addr = Ipv4Addr::new(IPV4_FIVE.0, IPV4_FIVE.1, IPV4_FIVE.2, IPV4_FIVE.3);
        let masked_ip_be = super::mask_ipv4_be(ipv4_addr) as u64;

        let node_id = super::generate_compliant_id(masked_ip_be, 4, IPV4_FIVE_RAND);

        assert_eq!(node_id[0], IPV4_FIVE_BITS.0);
        assert_eq!(node_id[1], IPV4_FIVE_BITS.1);

        assert_eq!(node_id[2] & 0xF8, IPV4_FIVE_BITS.2);

        assert_eq!(node_id[19], IPV4_FIVE_RAND);
    }

    #[test]
    fn positive_is_compliant_ipv4_test_one() {
        let ip_addr = Ipv4Addr::new(IPV4_ONE.0, IPV4_ONE.1, IPV4_ONE.2, IPV4_ONE.3);
        let id = [IPV4_ONE_BITS.0,
                  IPV4_ONE_BITS.1,
                  IPV4_ONE_BITS.2,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  IPV4_ONE_RAND]
            .into();

        let masked_ip_be = super::mask_ipv4_be(ip_addr) as u64;
        assert!(super::is_compliant_addr(masked_ip_be, 4, id));
    }

    #[test]
    fn positive_is_compliant_ipv4_test_two() {
        let ip_addr = Ipv4Addr::new(IPV4_TWO.0, IPV4_TWO.1, IPV4_TWO.2, IPV4_TWO.3);
        let id = [IPV4_TWO_BITS.0,
                  IPV4_TWO_BITS.1,
                  IPV4_TWO_BITS.2,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  IPV4_TWO_RAND]
            .into();

        let masked_ip_be = super::mask_ipv4_be(ip_addr) as u64;
        assert!(super::is_compliant_addr(masked_ip_be, 4, id));
    }

    #[test]
    fn positive_is_compliant_ipv4_test_three() {
        let ip_addr = Ipv4Addr::new(IPV4_THREE.0, IPV4_THREE.1, IPV4_THREE.2, IPV4_THREE.3);
        let id = [IPV4_THREE_BITS.0,
                  IPV4_THREE_BITS.1,
                  IPV4_THREE_BITS.2,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  IPV4_THREE_RAND]
            .into();

        let masked_ip_be = super::mask_ipv4_be(ip_addr) as u64;
        assert!(super::is_compliant_addr(masked_ip_be, 4, id));
    }

    #[test]
    fn positive_is_compliant_ipv4_test_four() {
        let ip_addr = Ipv4Addr::new(IPV4_FOUR.0, IPV4_FOUR.1, IPV4_FOUR.2, IPV4_FOUR.3);
        let id = [IPV4_FOUR_BITS.0,
                  IPV4_FOUR_BITS.1,
                  IPV4_FOUR_BITS.2,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  IPV4_FOUR_RAND]
            .into();

        let masked_ip_be = super::mask_ipv4_be(ip_addr) as u64;
        assert!(super::is_compliant_addr(masked_ip_be, 4, id));
    }

    #[test]
    fn positive_is_compliant_ipv4_test_five() {
        let ip_addr = Ipv4Addr::new(IPV4_FIVE.0, IPV4_FIVE.1, IPV4_FIVE.2, IPV4_FIVE.3);
        let id = [IPV4_FIVE_BITS.0,
                  IPV4_FIVE_BITS.1,
                  IPV4_FIVE_BITS.2,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  IPV4_FIVE_RAND]
            .into();

        let masked_ip_be = super::mask_ipv4_be(ip_addr) as u64;
        assert!(super::is_compliant_addr(masked_ip_be, 4, id));
    }
}
