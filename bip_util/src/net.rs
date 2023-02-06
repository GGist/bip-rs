use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4};

/// Abstraction of some ip address.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum IpAddr {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}

impl IpAddr {
    /// Create a new IpAddr from the given SocketAddr.
    pub fn from_socket_addr(sock_addr: SocketAddr) -> IpAddr {
        match sock_addr {
            SocketAddr::V4(v4_sock_addr) => IpAddr::V4(*v4_sock_addr.ip()),
            SocketAddr::V6(v6_sock_addr) => IpAddr::V6(*v6_sock_addr.ip()),
        }
    }
}

/// Get the default route ipv4 socket.
pub fn default_route_v4() -> SocketAddr {
    let v4_addr = Ipv4Addr::new(0, 0, 0, 0);
    let v4_sock = SocketAddrV4::new(v4_addr, 0);

    SocketAddr::V4(v4_sock)
}
