use std::io::net::udp::UdpSocket;
use std::io::net::ip::{SocketAddr, Ipv4Addr};

// http://upnp.org/sdcps-and-certification/standards/sdcps/

static SEARCH: &'static [u8] = b"M-SEARCH * HTTP/1.1\r
HOST: 239.255.255.250:1900\r
MAN: \"ssdp:discover\"\r
MX: 10\r
ST: ssdp:all\r
\r\n";

pub enum UPnP {
	Device(),
	Service()
}

pub struct UPnPDevice {
    pub data: String
}

impl UPnP {
    // Syncronous operation that will query 
    pub fn search(src_addr: SocketAddr) -> IoResult<Vec<UPnP>> {
        let dst = SocketAddr{ ip: Ipv4Addr(239, 255, 255, 250), port: 1900 };
		
        let mut udp = try!(UdpSocket::bind(src_addr));
		udp.set_read_timeout(Some(15000));
        udp.send_to(SEARCH, dst);
		
        let mut entries: Vec<UPnP> = Vec::new();
        loop {
            let mut buf = [0u8,..3000];
            
            match udp.recv_from(buf) {
                Ok(n) => {
                    let mut string = String::new();
                    for &i in buf.iter().take_while(|&&n| n != 0) {
                        string.push(i as char);
                    }
                    entries.push(UPnP{ data: string });
                },
                Err(e) => {
                    break;
                }
            };
        }
        
        entries
    }
}