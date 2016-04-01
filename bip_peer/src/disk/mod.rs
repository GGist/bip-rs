use mio::{Sender};

enum EDiskResponse {
    TestRequest
}

/// Manages memory used to hold pieces as well as storing and retrieving them from disk.
///
/// 
struct DiskManager {
    clients: Vec<Box<TDiskResponse>>
}

impl DiskManager {
    pub fn new(region_len: u32, max_regions: u32) -> DiskManager {
        unimplemented!();
    }
    
    pub fn register(&mut self, sender: Box<TDiskResponse>) {
        self.clients.push(sender);
    }
    
    /*
    pub fn request_block() -> TID
    
    pub fn access_block(id: TID)*/
}

//----------------------------------------------------------------------------//

trait TDiskResponse {
    fn send(&mut self, request: EDiskResponse);
}

struct SDiskResponse<T> where T: Send {
    send: Sender<T>
}

impl<T> SDiskResponse<T> where T: Send {
    pub fn new(send: Sender<T>) -> SDiskResponse<T> {
        SDiskResponse{ send: send }
    }
}

impl<T> TDiskResponse for SDiskResponse<T> where T: From<EDiskResponse> + Send {
    fn send(&mut self, request: EDiskResponse) {
        self.send.send(T::from(request));
    }
}

//----------------------------------------------------------------------------//