//! Utilities used throughout the library.

use std::borrow::{Borrow, ToOwned};
use std::collections::{HashMap, BTreeMap};
use std::hash::{Hash};
use std::io::{Result, Error, ErrorKind};
use std::net::{self, UdpSocket, Ipv4Addr, SocketAddr};

use rand;

const UNUSED_PORT_START: u16 = 1024;
const UNUSED_PORT_END: u16 = 49151;

/// Returns a list of all local IPv4 Addresses.
pub fn ipv4_net_addrs() -> Result<Vec<Ipv4Addr>> {
    let sock_iter = try!(net::lookup_host(""));
    
    let ipv4_list = sock_iter.filter_map(|addr|
        match addr {
            Ok(SocketAddr::V4(n)) => Some(*n.ip()),
            _                     => None
        }
    ).collect();
    
    Ok(ipv4_list)
}

/// Try to bind to a UDP port within the minimum and maximum range.
pub fn try_bind_udp(ip: Ipv4Addr) -> Result<UdpSocket> {
    try_range_udp(ip, UNUSED_PORT_START, UNUSED_PORT_END).map_err( |_|
        Error::new(ErrorKind::Other, "Could Not Bind To Any Ports")
    )
}

/// Try to bind to a UDP port within the range [start,end].
pub fn try_range_udp(ip: Ipv4Addr, start: u16, end: u16) -> Result<UdpSocket> {
    if start < UNUSED_PORT_START || start > UNUSED_PORT_END {
        return Err(Error::new(ErrorKind::Other, "Start Port Range Is Not In Bounds [1024,49151]"))
    } else if end < UNUSED_PORT_START || end > UNUSED_PORT_END {
        return Err(Error::new(ErrorKind::Other, "End Port Range Is Not In Bounds [1024,49151]"))
    }
    
    for i in start..(end + 1) {
        if let Ok(udp_sock) = UdpSocket::bind((ip, i)) {
            return Ok(udp_sock)
        }
    }
    
    Err(Error::new(ErrorKind::Other, "Could Not Bind To A Port Within The Range Specified"))
}

/// Applies a Fisher-Yates shuffle on the given list.
pub fn fisher_shuffle<T: Copy>(list: &mut [T]) {
    for i in 0..list.len() {
        let swap_index = (rand::random::<usize>() % (list.len() - i)) + i;
        
        let temp = list[i];
        list[i] = list[swap_index];
        list[swap_index] = temp;
    }
}

//----------------------------------------------------------------------------//

/// Trait for working with generic map data structures.
pub trait Dictionary<K, V> where K: Borrow<str> {
    /// Convert the dictionary to an unordered list of key/value pairs.
    fn to_list<'a>(&'a self) -> Vec<(&'a K, &'a V)>;

    /// Lookup a value in the dictionary.
    fn lookup<'a>(&'a self, key: &str) -> Option<&'a V>;

    /// Insert a key/value pair into the dictionary.
    fn insert(&mut self, key: K, value: V) -> Option<V>;
}

impl<K, V> Dictionary<K, V> for HashMap<K, V> where K: Hash + Eq + Borrow<str> {
    fn to_list<'a>(&'a self) -> Vec<(&'a K, &'a V)> {
        self.iter().collect()
    }

    fn lookup<'a>(&'a self, key: &str) -> Option<&'a V> {
        self.get(key)
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert(key, value)
    }
}

impl<K, V> Dictionary<K, V> for BTreeMap<K, V> where K: Ord + Borrow<str> {
    fn to_list<'a>(&'a self) -> Vec<(&'a K, &'a V)> {
        self.iter().collect()
    }

    fn lookup<'a>(&'a self, key: &str) -> Option<&'a V> {
        self.get(key)
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert(key, value)
    }
}