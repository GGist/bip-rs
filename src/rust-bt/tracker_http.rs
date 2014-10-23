pub struct HttpTracker {
    conn: TcpStream,
    dest: SocketAddr,
    peer_id: [u8,..20],
    info_hash: [u8,..20]
}

impl HttpTracker {
    //pub fn connect(url: &str, info_hash: &[u8]) -> IoResult<HttpTracker> {
        
    //}
}