use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr, Ipv6Addr};

use rand::{self};

use hash::{self};

/// Convert a 4 byte value to an array of 4 bytes.
pub fn four_bytes_to_array(bytes: u32) -> [u8; 4] {
    let eight_bytes = eight_bytes_to_array(bytes as u64);
    
    [eight_bytes[4], eight_bytes[5], eight_bytes[6], eight_bytes[7]]
}

/// Convert an 8 byte value to an array of 8 bytes.
pub fn eight_bytes_to_array(bytes: u64) -> [u8; 8] {
    [(bytes >> 56) as u8, (bytes >> 48) as u8, (bytes >> 40) as u8, (bytes >> 32) as u8,
    (bytes >> 24) as u8, (bytes >> 16) as u8, (bytes >> 8) as u8, (bytes >> 0) as u8]
}

/// Convert an ipv4 address to an array of 4 bytes big endian.
pub fn ipv4_to_bytes_be(v4_addr: Ipv4Addr) -> [u8; 4] {
    v4_addr.octets()
}

/// Convert an ipv6 address to an array of 16 bytes big endian.
pub fn ipv6_to_bytes_be(v6_addr: Ipv6Addr) -> [u8; 16] {
    let segments = v6_addr.segments();
    let mut bytes = [0u8; 16];
    
    for index in 0..bytes.len() {
        let segment_index = index / 2;
        
        let segment_byte_index = index % 2;
        let byte_shift_bits = 8 - (segment_byte_index * 8);
        
        let byte = (segments[segment_index] >> byte_shift_bits) as u8;
        
        bytes[index] = byte;
    }
    
    bytes
}


/*
/// Applies a Fisher-Yates shuffle on the given list.
pub fn fisher_shuffle<T: Copy>(list: &mut [T]) {
    for i in 0..list.len() {
        let swap_index = (rand::random::<usize>() % (list.len() - i)) + i;
        
        let temp = list[i];
        list[i] = list[swap_index];
        list[swap_index] = temp;
    }
}

/// Get the default route ipv4 socket.
pub fn default_route_v4() -> SocketAddr {
    let v4_addr = Ipv4Addr::new(0, 0, 0, 0);
    let v4_sock = SocketAddrV4::new(v4_addr, 0);
    
    SocketAddr::V4(v4_sock)
}
*/