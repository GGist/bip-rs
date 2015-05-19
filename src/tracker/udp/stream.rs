//! Streaming Data Over UDP.

use std::old_io::net::udp::{UdpSocket};
use std::old_io::net::ip::{SocketAddr, ToSocketAddr};
use std::old_io::{IoResult, TimedOut};

use util;

/// A UDP stream to a remote host.
pub struct UdpStream {
    conn: UdpSocket,
    src:  SocketAddr,
    dst:  SocketAddr
}

impl UdpStream {
    /// Creates a new UdpStream.
    pub fn new<T: ToSocketAddr>(mut conn: UdpSocket, dest: T) -> IoResult<UdpStream> {
        let src_sock = try!(conn.socket_name());
    
        Ok(UdpStream{ conn: conn, src: src_sock, dst: try!(dest.to_socket_addr()) })
    }
    
    pub fn local_sock(&self) -> SocketAddr {
        self.src
    }
    
    pub fn remote_sock(&self) -> SocketAddr {
        self.dst
    }
    
    /// Sends a request to the remote end of the stream and waits for a response.
    ///
    /// The timeout function should accept the zero based attempt index and return
    /// the number of milliseconds that we should wait for a response.
    pub fn request<T>(&mut self, src: &[u8], dst: &mut [u8], max_attempts: u32, timeout: T) -> IoResult<usize> 
        where T: Fn(u64) -> u64 {
        for i in range(0, max_attempts) {
            let curr_timeout = timeout(i as u64);
            
            try!(self.send(src, None));
            
            match self.recv(dst, Some(curr_timeout)) {
                Ok(bytes_recvd) => { return Ok(bytes_recvd) },
                Err(e)          => println!("{:?}", e)
            };
        }
        
        Err(util::simple_ioerror(TimedOut, "No Response After Maximum Attempts"))
    }
    
    /// Sends data to the remote end of the stream.
    pub fn send(&mut self, buf: &[u8], timeout: Option<u64>) -> IoResult<()> {
        self.conn.set_write_timeout(timeout);
        
        self.conn.send_to(buf, self.dst)
    }

    /// Receives data from the remote end of the stream. This function makes no
    /// guarantees as to the state of the buffer passed in other than on an Ok()
    /// result, the bytes up to the returned usize were sent by the remote host.
    pub fn recv(&mut self, buf: &mut [u8], timeout: Option<u64>) -> IoResult<usize> {
        let mut recvd_from_dest = false;
        
        // Not Resetting This In The Loop Because Client Shouldn't Care If We 
        // Got A Message From Someone Other Than The Remote Host.
        self.conn.set_read_timeout(timeout);
        
        let mut bytes_recvd = 0;
        while !recvd_from_dest {
            let (bytes, sender) = try!(self.conn.recv_from(buf));
            bytes_recvd = bytes;
            
            recvd_from_dest = sender == self.dst;
        }
        
        Ok(bytes_recvd)
    }
}

impl Writer for UdpStream {
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.send(buf, None)
    }
}

impl Reader for UdpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.recv(buf, None)
    }
}