use std::{rand, i64};
use std::io::net::udp::{UdpSocket, UdpStream};
use std::io::net::tcp::{TcpStream};
use std::io::net::ip::{SocketAddr, Ipv4Addr};
use std::io::{IoResult, BufWriter, BufReader, ConnectionFailed, EndOfFile, OtherIoError, InvalidInput};

use util;
use tracker::{AnnounceInfo, Tracker};

static MAX_ATTEMPTS: uint = 8;

pub struct UdpTracker {
    conn: UdpStream,
    peer_id: [u8,..20],
    info_hash: [u8,..20]
}

impl UdpTracker {
    /// Creates a new UdpTracker object.
    ///
    /// This function will send out a connect request on all available IPv4 
    /// interfaces and will use the interface that receives a response first.
    pub fn new(url: &str, info_hash: &[u8]) -> IoResult<UdpTracker> {
        let dest_sock = try!(util::get_sockaddr(url));
        
        if (info_hash.len() != 20) {
            return Err(util::get_error(InvalidInput, "Invalid Size For info_hash"));
        }
        
        let mut fixed_hash = [0u8,..20];
        for (dst, &src) in fixed_hash.iter_mut().zip(info_hash.iter()) {
            *dst = src;
        }
        
        // If too many net interfaces are found, it could be bad spawning a kernel thread for each of them
        let ip_addrs = try!(util::get_net_addrs());
        let (tx, rx) = channel();
        { // Need to move tx in scope so that it gets destroyed before receiving
            let tx = tx;
            for i in ip_addrs.into_iter() {
                let tx = tx.clone();
                spawn(proc() {
                    let mut udp_stream = match util::get_udp_sock(SocketAddr{ ip: i, port: 6881 }, 9) {
                        Ok(n) => n.connect(dest_sock),
                        Err(_) => return ()
                    };
					
                    match UdpTracker::connect_request(&mut udp_stream) {
                        Ok(n) => tx.send(udp_stream),
                        Err(n) => ()
                    }
                });
            }
        }
        
        let udp_stream = try!(rx.recv_opt().or_else( |_|
            Err(util::get_error(ConnectionFailed, "Could Not Communicate On Any IPv4 Interfaces"))
        ));
        Ok(UdpTracker{ conn: udp_stream, 
            peer_id: util::gen_peer_id(), 
            info_hash: fixed_hash }
        )
    }
    
    fn send_request(udp: &mut UdpStream, send: &[u8], recv: &mut [u8]) -> IoResult<uint> {
        let mut attempt = 0;

        let mut bytes_read = 0;
        while attempt < MAX_ATTEMPTS {
            try!(udp.write(send));
            
            let wait_seconds = util::get_udp_wait(attempt) * 1000;
            udp.as_socket(|udp| {
                udp.set_read_timeout(Some(wait_seconds))
            });
            
            match udp.read(recv) {
                Ok(bytes)  => { 
                    bytes_read = bytes; 
                    break; 
                },
                Err(_) => { attempt += 1; }
            };
        }
        if (attempt == MAX_ATTEMPTS) {
            return Err(util::get_error(ConnectionFailed, "No Connection Response From Server"));
        }
        
        Ok(bytes_read)
    }
    
    fn connect_request(udp: &mut UdpStream) -> IoResult<i64> {
        let mut send_bytes = [0u8,..16];
        let send_trans_id = rand::random::<i32>();
        
        { // Limit Lifetime Of Writer Object
            let mut send_writer = BufWriter::new(send_bytes);
            
            send_writer.write_be_i64(0x41727101980); // Part Of The Standard
            send_writer.write_be_i32(0); // Connect Request
            send_writer.write_be_i32(send_trans_id); // Verify This In The Response
        }
        
        let mut recv_bytes = [0u8,..16];
        let bytes_read = try!(UdpTracker::send_request(udp, send_bytes, recv_bytes));
        if (bytes_read != recv_bytes.len()) {
            return Err(util::get_error(EndOfFile, "Didn't Receive All 16 Bytes From Tracker"));
        }
        
        let mut recv_reader = BufReader::new(recv_bytes);
        if (try!(recv_reader.read_be_i32()) != 0) {
            return Err(util::get_error(OtherIoError, "Tracker Responded To A Different Action (Not Connect)"));
        }
        if (try!(recv_reader.read_be_i32()) != send_trans_id) {
            return Err(util::get_error(OtherIoError, "Tracker Did Not Send Us A Matching Transaction Id"));
        }
        
        recv_reader.read_be_i64()
    }
    
    //fn scrape_request() -> {
    
    //}
}

impl Tracker for UdpTracker {
	fn socket_name(&mut self) -> IoResult<SocketAddr> {
		self.conn.as_socket(|&udp| {
			udp.socket_name()
		})
	}

    fn announce(&mut self, total_size: uint) -> IoResult<AnnounceInfo> {
        let connect_id = try!(UdpTracker::connect_request(&mut self.conn));
        let send_trans_id = rand::random::<i32>();
        
        let mut send_bytes = [0u8,..98];
        {
            let mut send_buf = BufWriter::new(send_bytes);
            
            send_buf.write_be_i64(connect_id);
            send_buf.write_be_i32(1); // Announce Request
            send_buf.write_be_i32(send_trans_id); // Random For Each Request
            send_buf.write(self.info_hash); // Identifies The Torrent File
            send_buf.write(self.peer_id); // Self Designated Peer Id
            send_buf.write_be_i64(0); // Bytes Downloaded So Far
            send_buf.write_be_i64(total_size as i64);
            send_buf.write_be_i64(0); // Bytes Uploaded So Far
            send_buf.write_be_i32(0); // Specific Event
            send_buf.write_be_i32(0); // IPv4 Address (0 For Source Address)
            send_buf.write_be_i32(12); // Key (Not Sure Yet)
            send_buf.write_be_i32(-1); // Number Of Clients To Return (-1 Default)
            send_buf.write_be_i16(6882); // Port For Other Clients To Connect
        }
        
        let mut recv_bytes = [0u8,..10000];
        try!(UdpTracker::send_request(&mut self.conn, send_bytes, recv_bytes));
        
        let mut recv_reader = BufReader::new(recv_bytes);
        
        if (try!(recv_reader.read_be_i32()) != 1) {
            return Err(util::get_error(OtherIoError, "Tracker Responded To A Different Action (Not Announce)"));
        }
        if (try!(recv_reader.read_be_i32()) != send_trans_id) {
            return Err(util::get_error(OtherIoError, "Tracker Responded With A Different Transaction Id"));
        }
        
        let interval = try!(recv_reader.read_be_i32());
        let leechers = try!(recv_reader.read_be_i32());
        let seeders = try!(recv_reader.read_be_i32());
        let mut peers: Vec<SocketAddr> = Vec::with_capacity(leechers as uint + seeders as uint);
        
        for _ in range(0, seeders + leechers) {
            peers.push(SocketAddr{ ip: Ipv4Addr(try!(recv_reader.read_u8()), 
                try!(recv_reader.read_u8()), try!(recv_reader.read_u8()), 
                try!(recv_reader.read_u8())), port: try!(recv_reader.read_be_u16()) }
            );
        }
        
        Ok(AnnounceInfo{ interval: interval, leechers: leechers, 
            seeders: seeders, peers: peers }
        )
    }
}