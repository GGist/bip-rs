use std::{rand};
use std::thread::{Thread};
use std::time::duration::{Duration};
use std::sync::{Arc};
use std::sync::mpsc::{self, Receiver};
use std::sync::atomic::{AtomicBool, Ordering};
use std::io::timer::{Timer};
use std::io::net::udp::{UdpSocket};
use std::io::net::ip::{SocketAddr, Ipv4Addr, IpAddr};
use std::io::{IoResult, BufWriter, BufReader, ConnectionFailed, EndOfFile, OtherIoError, InvalidInput};

use util;
use tracker::{AnnounceInfo, ScrapeInfo, Tracker};

static MAX_ATTEMPTS: uint = 8;

pub struct UdpTracker {
    conn: UdpSocket,
    tracker: SocketAddr,
    peer_id: [u8; 20],
    info_hash: [u8; 20],
    conn_id: i64,
    conn_id_expire: Receiver<()>
}

impl UdpTracker {
    /// Creates a new UdpTracker object.
    ///
    /// This function will send out a connect request on all available IPv4 
    /// interfaces and will use the interface that receives a response first.
    pub fn new(url: &str, info_hash: &[u8]) -> IoResult<UdpTracker> {
        let dest_sock = try!(util::get_sockaddr(url));
        
        if info_hash.len() != 20 {
            return Err(util::get_error(InvalidInput, "Invalid Size For info_hash"));
        }
        
        let mut fixed_hash = [0u8; 20];
        for (dst, &src) in fixed_hash.iter_mut().zip(info_hash.iter()) {
            *dst = src;
        }
        
        // List of local ip addresses to test for a connection on
        let ip_addrs = try!(util::get_net_addrs());
        
        let recvd_response = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        // If too many net interfaces are found, it could be bad spawning a kernel thread for each of them
        { // Need to move tx in scope so that it gets destroyed before receiving
            let tx = tx;
            for i in ip_addrs.into_iter() {
                let tx = tx.clone();
                let recvd_response = recvd_response.clone();
                
                Thread::spawn(move || {
                    let mut curr_attempt = 0;
                    let mut udp_sock = match util::get_udp_sock(SocketAddr{ ip: i, port: 6881 }, 9) {
                        Ok(n) => n,
                        Err(_) => return ()
                    };
                    
                    while curr_attempt < MAX_ATTEMPTS && !recvd_response.load(Ordering::Relaxed) {
                        match connect_request(&mut udp_sock, &dest_sock, 1) {
                            Ok((id, expire)) => return tx.send((udp_sock, id, expire)).unwrap(),
                            Err(_) => curr_attempt += 1
                        };
                    }
                }).detach();
            }
        }
        
        let (udp_sock, id, expire) = try!(rx.recv().map_err( |_|
            util::get_error(ConnectionFailed, "Could Not Communicate On Any IPv4 Interfaces")
        ));
        recvd_response.store(true, Ordering::Relaxed);
        
        Ok(UdpTracker{ conn: udp_sock,
            tracker: dest_sock,
            peer_id: util::gen_peer_id(),
            info_hash: fixed_hash,
            conn_id: id,
            conn_id_expire: expire }
        )
    }
    
    /// Checks if our connection id is valid and updates it is necessary.
    ///
    /// This is a blocking operation.
    fn check_connection_id(&mut self) -> IoResult<()> {
        match self.conn_id_expire.try_recv() {
            Ok(_) => {
                let (new_id, new_expire) = try!(connect_request(&mut self.conn, &self.tracker, MAX_ATTEMPTS));
                self.conn_id = new_id;
                self.conn_id_expire = new_expire;
            },
            Err(_) => ()
        };
        
        Ok(())
    }
    
    /// Parameterized announce request adhering to the UdpTracker Protocol.
    ///
    /// This is a blocking operation.
    fn announce(&mut self, downloaded: i64, left: i64, uploaded: i64, event: i32, port: i16) -> IoResult<AnnounceInfo> {
        try!(self.check_connection_id());
        let send_trans_id = rand::random::<i32>();
        
        let mut send_bytes = [0u8; 98];
        {
            let mut send_buf = BufWriter::new(&mut send_bytes);
            
            try!(send_buf.write_be_i64(self.conn_id));  // Our Connection Id
            try!(send_buf.write_be_i32(1));             // This Is An Announce Request
            try!(send_buf.write_be_i32(send_trans_id)); // Random For Each Request
            try!(send_buf.write(&self.info_hash));      // Identifies The Torrent File
            try!(send_buf.write(&self.peer_id));        // Self Designated Peer Id
            try!(send_buf.write_be_i64(downloaded));    // Bytes Downloaded So Far
            try!(send_buf.write_be_i64(left));          // Bytes Needed
            try!(send_buf.write_be_i64(uploaded));      // Bytes Uploaded So Far
            try!(send_buf.write_be_i32(event));         // Specific Event
            try!(send_buf.write_be_i32(0));             // IPv4 Address (0 For Source Address)
            try!(send_buf.write_be_i32(12));            // Key (Helps With Endianness For Tracker?)
            try!(send_buf.write_be_i32(-1));            // Number Of Clients To Return (-1 Default)
            try!(send_buf.write_be_i16(port));          // Port For Other Clients To Connect (Needs To Be Port Forwarded Behind NAT)
        }
        
        let mut recv_bytes = [0u8; 10000];
        try!(send_request(&mut self.conn, &self.tracker, &send_bytes, &mut recv_bytes, MAX_ATTEMPTS));
        
        let mut recv_reader = BufReader::new(&recv_bytes);
        
        if try!(recv_reader.read_be_i32()) != 1 {
            return Err(util::get_error(OtherIoError, "Tracker Responded To A Different Action (Not Announce)"));
        }
        if try!(recv_reader.read_be_i32()) != send_trans_id {
            return Err(util::get_error(OtherIoError, "Tracker Responded With A Different Transaction Id"));
        }
        
        let interval = try!(recv_reader.read_be_i32()) as i64;
        let leechers = try!(recv_reader.read_be_i32());
        let seeders = try!(recv_reader.read_be_i32());
        let mut peers: Vec<SocketAddr> = Vec::with_capacity(leechers as uint + seeders as uint);
        
        for _ in range(0, seeders + leechers) {
            peers.push(SocketAddr{ ip: Ipv4Addr(try!(recv_reader.read_u8()), 
                try!(recv_reader.read_u8()), try!(recv_reader.read_u8()), 
                try!(recv_reader.read_u8())), port: try!(recv_reader.read_be_u16()) }
            );
        }
        
        Ok(AnnounceInfo{ interval: try!(Timer::new()).oneshot(Duration::seconds(interval)), 
                         leechers: leechers, seeders: seeders, peers: peers }
        )
    }
}

/// Sends a request on udp using send as the buffer of bytes to send and dumping the
/// response into recv. This method uses the standard UDP Tracker time out algorithm
/// to wait for a response from the server before failing.
///
/// This is a blocking operation.
fn send_request(udp: &mut UdpSocket, dst: &SocketAddr, send: &[u8], recv: &mut [u8], attempts: uint) -> IoResult<uint> {
    let mut attempt = 0;

    let mut bytes_read = 0;
    while attempt < attempts {
        try!(udp.send_to(send, *dst));

        let wait_ms = util::get_udp_wait(attempt) * 1000;
        udp.set_read_timeout(Some(wait_ms));

        match udp.recv_from(recv) {
            Ok((bytes, _))  => { 
                bytes_read = bytes; 
                break; 
            },
            Err(_) => { attempt += 1; }
        };
    }
    if attempt == attempts {
        return Err(util::get_error(ConnectionFailed, "No Connection Response From Server"));
    }

    Ok(bytes_read)
}

/// Sends a request on udp with a message that is asking for a connection id from
/// the server. This connection id is required in order to send any sort of data
/// to the server so that it can map our ip address to the connection id to prevent
/// spoofing later on. This connection id is valid until the receiver is activated.
///
/// This is a blocking operation.
fn connect_request(udp: &mut UdpSocket, dst: &SocketAddr, attempts: uint) -> IoResult<(i64, Receiver<()>)> {
    let mut send_bytes = [0u8; 16];
    let send_trans_id = rand::random::<i32>();

    { // Limit Lifetime Of Writer Object
        let mut send_writer = BufWriter::new(&mut send_bytes);

        try!(send_writer.write_be_i64(0x41727101980)); // Part Of The Standard
        try!(send_writer.write_be_i32(0)); // Connect Request
        try!(send_writer.write_be_i32(send_trans_id)); // Verify This In The Response
    }

    let mut recv_bytes = [0u8; 16];
    let bytes_read = try!(send_request(udp, dst, &send_bytes, &mut recv_bytes, attempts));
    if bytes_read != recv_bytes.len() {
        return Err(util::get_error(EndOfFile, "Didn't Receive All 16 Bytes From Tracker"));
    }

    let mut recv_reader = BufReader::new(&recv_bytes);
    if try!(recv_reader.read_be_i32()) != 0 {
        return Err(util::get_error(OtherIoError, "Tracker Responded To A Different Action (Not Connect)"));
    }
    if try!(recv_reader.read_be_i32()) != send_trans_id {
        return Err(util::get_error(OtherIoError, "Tracker Did Not Send Us A Matching Transaction Id"));
    }

    let conn_id = try!(recv_reader.read_be_i64());
    // Connection IDs Are Valid Up To 1 Minute After Being Created
    let conn_id_expire = try!(Timer::new()).oneshot(Duration::minutes(1));
    // TODO: Find a better way to keep Timer objects around (maybe make them static?)

    Ok((conn_id, conn_id_expire))
}

impl Tracker for UdpTracker {
    fn local_ip(&mut self) -> IoResult<IpAddr> {
        Ok((try!(self.conn.socket_name())).ip)
    }

    fn scrape(&mut self) -> IoResult<ScrapeInfo> {
        try!(self.check_connection_id());
        let send_trans_id = rand::random::<i32>();
        
        let mut send_bytes = [0u8; 36];
        {
            let mut send_buf = BufWriter::new(&mut send_bytes);
            
            try!(send_buf.write_be_i64(self.conn_id));  // Our Connection Id
            try!(send_buf.write_be_i32(2));             // This Is A Scrape Request
            try!(send_buf.write_be_i32(send_trans_id)); // Random For Each Request
            try!(send_buf.write(&self.info_hash));      // Identifies The Torrent File
        }
        
        let mut recv_bytes = [0u8; 10000];
        try!(send_request(&mut self.conn, &self.tracker, &send_bytes, &mut recv_bytes, MAX_ATTEMPTS));
        
        let mut recv_reader = BufReader::new(&recv_bytes);
        
        let action = try!(recv_reader.read_be_i32());
        if action != 2 {
            if action == 3 { // We Were Sent An Error Response, Read It
                // TODO: When Redoing Error Handling, Use The Error String In The Response.
                return Err(util::get_error(OtherIoError, "Tracker Responded With An Error Code"));
            } else {         // They Sent Less Than 8 Bytes
                return Err(util::get_error(OtherIoError, "Tracker Responded With An Incomplete Response"));
            }
        }
        
        if try!(recv_reader.read_be_i32()) != send_trans_id {
            return Err(util::get_error(OtherIoError, "Tracker Responded With A Different Transaction Id"));
        }
        
        let seeders = try!(recv_reader.read_be_i32());
        let downloads = try!(recv_reader.read_be_i32());
        let leechers = try!(recv_reader.read_be_i32());
        
        Ok(ScrapeInfo{ leechers: leechers, seeders: seeders, downloads: downloads})
    }

    fn start_announce(&mut self, total_size: uint) -> IoResult<AnnounceInfo> {
        self.announce(0, total_size as i64, 0, 0, 6882)
    }

    fn update_announce(&mut self, total_down: uint, total_left: uint, total_up: uint) -> IoResult<AnnounceInfo> {
        self.announce(total_down as i64, total_left as i64, total_up as i64, 2, 6882)
    }

    fn stop_announce(&mut self, total_down: uint, total_left: uint, total_up: uint) -> IoResult<()> {
        try!(self.announce(total_down as i64, total_left as i64, total_up as i64, 3, 6882));
        Ok(())
    }

    fn complete_announce(&mut self, total_bytes: uint) -> IoResult<()> {
        try!(self.announce(total_bytes as i64, 0, total_bytes as i64, 1, 6882));
        Ok(())
    }
}