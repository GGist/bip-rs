use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

/// Convert a 4 byte value to an array of 4 bytes.
pub fn four_bytes_to_array(bytes: u32) -> [u8; 4] {
    let eight_bytes = eight_bytes_to_array(bytes as u64);

    [
        eight_bytes[4],
        eight_bytes[5],
        eight_bytes[6],
        eight_bytes[7],
    ]
}

/// Convert an 8 byte value to an array of 8 bytes.
pub fn eight_bytes_to_array(bytes: u64) -> [u8; 8] {
    [
        (bytes >> 56) as u8,
        (bytes >> 48) as u8,
        (bytes >> 40) as u8,
        (bytes >> 32) as u8,
        (bytes >> 24) as u8,
        (bytes >> 16) as u8,
        (bytes >> 8) as u8,
        bytes as u8,
    ]
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

// Convert a port to an array of 2 bytes big endian.
pub fn port_to_bytes_be(port: u16) -> [u8; 2] {
    [(port >> 8) as u8, (port >> 0) as u8]
}

/// Convert a v4 socket address to an array of 6 bytes big endian.
pub fn sock_v4_to_bytes_be(v4_sock: SocketAddrV4) -> [u8; 6] {
    let mut sock_bytes = [0u8; 6];

    let ip_bytes = ipv4_to_bytes_be(*v4_sock.ip());
    let port_bytes = port_to_bytes_be(v4_sock.port());

    for (src, dst) in ip_bytes
        .iter()
        .chain(port_bytes.iter())
        .zip(sock_bytes.iter_mut())
    {
        *dst = *src;
    }

    sock_bytes
}

/// Convert a v6 socket address to an array of 18 bytes big endian.
pub fn sock_v6_to_bytes_be(v6_sock: SocketAddrV6) -> [u8; 18] {
    let mut sock_bytes = [0u8; 18];

    let ip_bytes = ipv6_to_bytes_be(*v6_sock.ip());
    let port_bytes = port_to_bytes_be(v6_sock.port());

    for (src, dst) in ip_bytes
        .iter()
        .chain(port_bytes.iter())
        .zip(sock_bytes.iter_mut())
    {
        *dst = *src;
    }

    sock_bytes
}

/// Convert an array of 4 bytes big endian to an ipv4 address.
pub fn bytes_be_to_ipv4(bytes: [u8; 4]) -> Ipv4Addr {
    Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])
}

/// Convert an array of 16 bytes big endian to an ipv6 address.
pub fn bytes_be_to_ipv6(bytes: [u8; 16]) -> Ipv6Addr {
    let mut combined_bytes = [0u16; 8];

    for (index, &value) in bytes.iter().enumerate() {
        let combined_index = index / 2;
        let should_shift = index % 2 == 0;

        let adjusted_value = if should_shift {
            (value as u16) << 8
        } else {
            value as u16
        };

        combined_bytes[combined_index] |= adjusted_value;
    }

    Ipv6Addr::new(
        combined_bytes[0],
        combined_bytes[1],
        combined_bytes[2],
        combined_bytes[3],
        combined_bytes[4],
        combined_bytes[5],
        combined_bytes[6],
        combined_bytes[7],
    )
}

/// Convert an array of 2 bytes big endian to a port.
pub fn bytes_be_to_port(bytes: [u8; 2]) -> u16 {
    let (high_byte, low_byte) = (bytes[0] as u16, bytes[1] as u16);

    (high_byte << 8) | low_byte
}

/// Convert an array of 6 bytes big endian to a v4 socket address.
pub fn bytes_be_to_sock_v4(bytes: [u8; 6]) -> SocketAddrV4 {
    let ip_bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
    let port_bytes = [bytes[4], bytes[5]];

    let (ip, port) = (bytes_be_to_ipv4(ip_bytes), bytes_be_to_port(port_bytes));

    SocketAddrV4::new(ip, port)
}

/// Convert an array of 18 bytes big endian to a v6 socket address.
pub fn bytes_be_to_sock_v6(bytes: [u8; 18]) -> SocketAddrV6 {
    let mut ip_bytes = [0u8; 16];
    let port_bytes = [bytes[16], bytes[17]];

    for (src, dst) in bytes.iter().take(16).zip(ip_bytes.iter_mut()) {
        *dst = *src;
    }

    let (ip, port) = (bytes_be_to_ipv6(ip_bytes), bytes_be_to_port(port_bytes));

    SocketAddrV6::new(ip, port, 0, 0)
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

    #[test]
    fn positive_port_to_bytes_be() {
        let port = 0xAB00 | 0x00CD;

        let received = super::port_to_bytes_be(port);
        let expected = [0xAB, 0xCD];

        assert_eq!(received, expected);
    }

    #[test]
    fn positive_sock_v4_to_bytes_be() {
        let sock_addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1600);

        let received = super::sock_v4_to_bytes_be(sock_addr);
        let expected = [127, 0, 0, 1, (1600 >> 8) as u8, (1600 >> 0) as u8];

        assert_eq!(received, expected);
    }

    #[test]
    fn positive_sock_v6_to_bytes_be() {
        let sock_addr = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 1821, 0, 0);

        let received = super::sock_v6_to_bytes_be(sock_addr);
        let expected = [
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
            1,
            (1821 >> 8) as u8,
            (1821 >> 0) as u8,
        ];

        assert_eq!(received, expected);
    }

    #[test]
    fn positive_four_bytes_to_array() {
        let bytes = [25, 26, 0, 60];
        let mut combined_bytes = 0u32;

        let mut shift = 32 - 8;
        for &byte in bytes.iter() {
            let shifted_byte = (byte as u32) << shift;
            combined_bytes |= shifted_byte;

            shift -= 8;
        }
        let result_bytes = super::four_bytes_to_array(combined_bytes);

        assert_eq!(bytes, result_bytes);
    }

    #[test]
    fn positive_eight_bytes_to_array() {
        let bytes = [25, 26, 0, 60, 43, 45, 65, 1];
        let mut combined_bytes = 0u64;

        let mut shift = 64 - 8;
        for &byte in bytes.iter() {
            let shifted_byte = (byte as u64) << shift;
            combined_bytes |= shifted_byte;

            shift -= 8;
        }
        let result_bytes = super::eight_bytes_to_array(combined_bytes);

        assert_eq!(bytes, result_bytes);
    }

    #[test]
    fn positive_ipv4_to_bytes_be() {
        let bytes = [127, 0, 0, 1];
        let ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);

        let result_bytes = super::ipv4_to_bytes_be(ip);

        assert_eq!(bytes, result_bytes);
    }

    #[test]
    fn positive_ipv6_to_bytes_be() {
        let bytes = [1, 0, 0, 0, 0, 0, 0, 1];
        let expected_bytes = [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

        let ip = Ipv6Addr::new(
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        );

        let result_bytes = super::ipv6_to_bytes_be(ip);

        assert_eq!(expected_bytes, result_bytes);
    }

    #[test]
    fn positive_bytes_be_to_ipv4() {
        let bytes = [127, 0, 0, 1];
        let expected_ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);

        let result_ip = super::bytes_be_to_ipv4(bytes);

        assert_eq!(expected_ip, result_ip);
    }

    #[test]
    fn positive_bytes_be_to_ipv6() {
        let bytes = [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let expected_ip = Ipv6Addr::new(1, 0, 0, 0, 0, 0, 0, 1);

        let result_ip = super::bytes_be_to_ipv6(bytes);

        assert_eq!(expected_ip, result_ip);
    }

    #[test]
    fn positive_bytes_be_to_port() {
        let bytes = [1, 1];

        let result_port = super::bytes_be_to_port(bytes);

        assert_eq!(257, result_port);
    }

    #[test]
    fn positive_bytes_be_to_sock_v4() {
        let bytes = [127, 0, 0, 1, 1, 1];
        let expected_sock = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 257);

        let result_sock = super::bytes_be_to_sock_v4(bytes);

        assert_eq!(expected_sock, result_sock);
    }

    #[test]
    fn positive_bytes_be_to_sock_v6() {
        let bytes = [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1];
        let expected_sock = SocketAddrV6::new(Ipv6Addr::new(1, 0, 0, 0, 0, 0, 0, 1), 257, 0, 0);

        let result_sock = super::bytes_be_to_sock_v6(bytes);

        assert_eq!(expected_sock, result_sock);
    }
}
