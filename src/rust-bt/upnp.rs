use util;
use std::path::BytesContainer;
use std::io::{IoResult, InvalidInput};
use std::io::net::udp::UdpSocket;
use std::io::net::ip::{SocketAddr, Ipv4Addr};

// http://upnp.org/sdcps-and-certification/standards/sdcps/

static SEARCH: &'static [u8] = b"M-SEARCH * HTTP/1.1\r
HOST: 239.255.255.250:1900\r
MAN: \"ssdp:discover\"\r
MX: 10\r
ST: ssdp:all\r
\r\n";

pub struct SearchReply {
    payload: String,
	location: (uint, uint),
	st: (uint, uint),
	usn: (uint, uint)
}

impl SearchReply {
    pub fn search(from_addr: SocketAddr) -> IoResult<Vec<SearchReply>> {
        let dst_addr = SocketAddr{ ip: Ipv4Addr(239, 255, 255, 250), port: 1900 };
        let mut udp_sock = try!(UdpSocket::bind(from_addr));
        
        udp_sock.set_read_timeout(Some(10000));
        udp_sock.send_to(SEARCH, dst_addr);
        
        let mut replies: Vec<SearchReply> = Vec::new();
        let mut reply_buf = [0u8,..1000];
        loop {
            match udp_sock.recv_from(reply_buf) {
                Ok(n) => {
                    let end_buf = try!(reply_buf.iter().position({ |&b|
                        b == 0u8
                    }).ok_or(util::get_error(InvalidInput, "UPnP Reply Corrupt")));
                    let payload: String = try!(reply_buf.slice_to(end_buf).container_as_str().ok_or(
                        util::get_error(InvalidInput, "UPnP Reply Not A Valid String")
                    )).into_string();

                    let location_start = try!(payload.as_slice().find_str("Location:").ok_or(
                        util::get_error(InvalidInput, "UPnP Reply Missing Location Field")
                    )) + 9;
                    let location_len = try!(payload.as_slice().chars().skip(location_start).position(|c| {
                        c == '\r'
                    }).ok_or(util::get_error(InvalidInput, "UPnP Reply Corrupt Location Field")));
                    
                    let st_start = try!(payload.as_slice().find_str("ST:").ok_or(
                        util::get_error(InvalidInput, "UPnP Reply Missing ST Field")
                    )) + 3;
                    let st_len = try!(payload.as_slice().chars().skip(st_start).position(|c| {
                        c == '\r'
                    }).ok_or(util::get_error(InvalidInput, "UPnP Reply Corrupt ST Field")));
                    
                    let usn_start = try!(payload.as_slice().find_str("USN:").ok_or(
                        util::get_error(InvalidInput, "UPnP Reply Missing USN Field")
                    ))+ 4;
                    let usn_len = try!(payload.as_slice().chars().skip(st_start).position(|c| {
                        c == '\r'
                    }).ok_or(util::get_error(InvalidInput, "UPnP Reply Corrupt USN Field")));

                    replies.push(SearchReply{ payload: payload, 
                        location: (location_start, location_len), 
                        st: (st_start, st_len), 
                        usn: (usn_start, usn_len) }
                    );
                },
                Err(_) => { break; }
            };
        }
        
        Ok(replies)
    }
    
    pub fn get_location<'a>(&'a self) -> &'a str {
        let (start, len) = self.location;
        
        self.payload[start..len + start]
    }
    
    pub fn get_st<'a>(&'a self) -> &'a str {
        let (start, len) = self.st;
        
        self.payload[start..len + start]
    }
    
    pub fn get_usn<'a>(&'a self) -> &'a str {
        let (start, len) = self.usn;
        
        self.payload[start..len + start]
    }
}

