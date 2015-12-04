use std::fmt::{self, Display, Formatter};
use std::io::{self, ErrorKind, Error};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs};
use std::vec::{IntoIter};

const UTORRENT_DHT:     (&'static str, u16) = ("router.utorrent.com", 6881);
const BITCOMET_DHT:     (&'static str, u16) = ("router.bitcomet.com", 6881);
const TRANSMISSION_DHT: (&'static str, u16) = ("dht.transmissionbt.com", 6881);

/// Enumerates different routers that can be used to bootstrap a dht.
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Router {
    /// Bootstrap server maintained by uTorrent.
    uTorrent,
    /// Bootstrap server maintained by BitComet.
    BitComet,
    /// Bootstrap server maintained by Transmission.
    Transmission,
    /// Custom bootstrap server.
    Custom(SocketAddr)
}

impl Router {
    /* TODO: USES DEPRECATED FUNCTIONS
    pub fn hostname(&self) -> io::Result<Cow<'static, str>> {
        match self {
            &Router::uTorrent     => Ok(UTORRENT_DHT.0.into_cow()),
            &Router::BitComet     => Ok(BITCOMET_DHT.0.into_cow()),
            &Router::Transmission => Ok(TRANSMISSION_DHT.0.into_cow()),
            &Router::Custom(addr) => {
                net::lookup_addr(&addr.ip()).map(|n| n.into_cow())
            }
        }
    }*/

    pub fn ipv4_addr(&self) -> io::Result<SocketAddrV4> {
        let addrs = try!(self.socket_addrs());
        
        addrs.filter_map(map_ipv4).next().ok_or(
            Error::new(ErrorKind::Other, "No IPv4 Addresses Found For Host")
        )
    }
    
    pub fn ipv6_addr(&self) -> io::Result<SocketAddrV6> {
        let addrs = try!(self.socket_addrs());
        
        addrs.filter_map(map_ipv6).next().ok_or(
            Error::new(ErrorKind::Other, "No IPv6 Addresses Found For Host")
        )
    }
    
    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        let mut addrs = try!(self.socket_addrs());
        
        addrs.next().ok_or(Error::new(ErrorKind::Other, "No SocketAddresses Found For Host"))
    }
    
    fn socket_addrs(&self) -> io::Result<IntoIter<SocketAddr>> {
        match self {
            &Router::uTorrent     => UTORRENT_DHT.to_socket_addrs(),
            &Router::BitComet     => BITCOMET_DHT.to_socket_addrs(),
            &Router::Transmission => TRANSMISSION_DHT.to_socket_addrs(),
            &Router::Custom(addr) => {
                // TODO: Wasteful, should check for Custom before calling function
                Ok(vec![addr].into_iter())
            }
        }
    }
}

fn map_ipv4(addr: SocketAddr) -> Option<SocketAddrV4> {
    match addr {
        SocketAddr::V4(n) => Some(n),
        SocketAddr::V6(_) => None
    }
}

fn map_ipv6(addr: SocketAddr) -> Option<SocketAddrV6> {
    match addr {
        SocketAddr::V4(_) => None,
        SocketAddr::V6(n) => Some(n)
    }
}

impl Display for Router {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            Router::uTorrent => {
                f.write_fmt(format_args!("{}:{}", UTORRENT_DHT.0, UTORRENT_DHT.1))
            },
            Router::BitComet => {
                f.write_fmt(format_args!("{}:{}", BITCOMET_DHT.0, BITCOMET_DHT.1))
            },
            Router::Transmission => {
                f.write_fmt(format_args!("{}:{}", TRANSMISSION_DHT.0, TRANSMISSION_DHT.1))
            },
            Router::Custom(n) => Display::fmt(&n, f)
        }
    }
}