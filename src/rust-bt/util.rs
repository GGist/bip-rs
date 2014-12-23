use regex::Regex;
use std::{str, rand};
use std::num::Int;
use std::io::{IoError, IoResult, IoErrorKind, InvalidInput, ConnectionFailed};
use std::io::net::addrinfo;
use std::io::net::udp::{UdpSocket};
use std::io::net::ip::{SocketAddr, Ipv4Addr, IpAddr, Ipv6Addr};

static URL_REGEX: Regex = regex!(r"\A(\w+)://([^ ]+?)(?::(\d+))?(/.*)");
static PEER_ID_PREFIX: &'static str = "RBT-0-1-1--";

#[deriving(Copy)]
pub enum Transport { TCP, UDP, HTTP }

/// Returns a list of all local IPv4 Addresses.
pub fn get_net_addrs() -> IoResult<Vec<IpAddr>> {
    let addr_list = try!(addrinfo::get_host_addresses(""));
    
    let addr_list = addr_list.into_iter().filter(|&addr|
        match addr {
            Ipv4Addr(..) => true,
            Ipv6Addr(..) => false
        }
    ).collect();
    
    Ok(addr_list)
}

/// Attempts to open a udp connection on addr. 
/// 
/// If the connection is unsuccessful, it will try again up to (attempts - 1)
/// times, incrementing the port for each attempt.
pub fn get_udp_sock(mut addr: SocketAddr, mut attempts: uint) -> IoResult<UdpSocket> {
    let mut udp_socket = UdpSocket::bind(addr);
    attempts -= 1;
    
    while udp_socket.is_err() && attempts > 0 {
        addr.port += 1;
        attempts -= 1;
        
        udp_socket = UdpSocket::bind(addr);
    }
    
    match udp_socket {
        Ok(n)  => Ok(n),
        _ => Err(get_error(ConnectionFailed, "Could Not Bind To A UDP Port"))
    }
}

/// The standard wait algorithm defined in the UDP Tracker Protocol. Returned value
/// is in seconds.
pub fn get_udp_wait(attempt: uint) -> u64 {
    (15 * 2i.pow(attempt)) as u64
}

/// Generates a peer id from a base identifier followed by random characters.
pub fn gen_peer_id() -> [u8,..20] {
    let mut bytes = [0u8, ..20];
    
    for (byte, pref) in bytes.iter_mut().zip(PEER_ID_PREFIX.chars()) {
        *byte = pref as u8;
    }
    
    for i in range(PEER_ID_PREFIX.len(), 20) {
        bytes[i] = rand::random::<char>() as u8;
    }
    
    bytes
}

/// Takes a url and returns the transport type that it specifies.
pub fn get_transport(url: &str) -> IoResult<Transport> {
    let trans_str = try!(try!(URL_REGEX.captures(url).ok_or(
        get_error(InvalidInput, "Transport Protocol Not Found In url")
    )).at(1).ok_or(
        get_error(InvalidInput, "Transport Protocol Not Found In url")
    ));
    
    if trans_str.len() == 0 {
        return Err(get_error(InvalidInput, "Transport Protocol Not Found In url"));
    }
        
    match trans_str {
        "http" => Ok(Transport::HTTP), 
        "tcp"  => Ok(Transport::TCP),
        "udp"  => Ok(Transport::UDP),
        _ => Err(get_error(InvalidInput, "Transport Protocol Not Found In url"))
    }
}

/// Returns the first found DNS entry as a SocketAddr for the specified url.
pub fn get_sockaddr(url: &str) -> IoResult<SocketAddr> {
    let captures = try!(URL_REGEX.captures(url).ok_or(
        get_error(InvalidInput, "Hostname And/Or Port Number Not Found In url")
    ));
    
    let (host_str, port_str) = (captures.at(2).unwrap_or(""), captures.at(3).unwrap_or(""));
    if host_str.len() == 0 || port_str.len() == 0 {
        return Err(get_error(InvalidInput, "Hostname And/Or Port Number Not Found In url"))
    }
    
    let host_ip = try!(addrinfo::get_host_addresses(host_str))[0];
    let port_num = try!(str::from_str(port_str).ok_or(
        get_error(InvalidInput, "Invalid Port Number Found In url")
    ));
    
    Ok(SocketAddr{ ip: host_ip, port: port_num })
}

/// Returns the path portion of a supplied url.
pub fn get_path(url: &str) -> IoResult<&str> {
    let path_str = match URL_REGEX.captures(url) {
        Some(n) => n.at(4).unwrap_or(""),
        None    => return Err(get_error(InvalidInput, "No Path Found In url"))
    };
    
    Ok(path_str)
}

/// Used to fill an IoError with a kind and desc, leaving detail empty.
pub fn get_error(err_type: IoErrorKind, msg: &'static str) -> IoError {
    IoError{ kind: err_type, desc: msg, detail: None }
}

#[cfg(test)]
mod tests {
	use super::{get_udp_wait, get_path};
	
	#[test]
	fn positive_get_path() {
		assert_eq!(get_path("http://test.com:80/test_path").unwrap(), "/test_path");
		
		assert_eq!(get_path("http://test.com/test_path").unwrap(), "/test_path");
	}
	
	#[test]
	fn positive_get_udp_wait() {
		assert_eq!(get_udp_wait(0), 15u64);
		
		assert_eq!(get_udp_wait(1), 30u64);
		
		assert_eq!(get_udp_wait(-1), 0u64);
	}
}