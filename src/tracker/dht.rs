pub struct DhtTracker {
    conn: UdpSocket,
    dest: SocketAddr,
    peer_id: [u8,..20],
    info_hash: [u8,..20]
}

impl DhtTracker {
    //pub fn connect(url: &str, info_hash: &[u8]) -> IoResult<DhtTracker> {
        
    //}
}