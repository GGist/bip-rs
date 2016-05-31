#![allow(unused)]

use std::sync::{Arc, Mutex};
use std::sync::mpsc::{SyncSender};

use bip_util::sender::{Sender};

use disk::worker::{WorkerMessage};
use token::{Token, TokenGenerator};

mod worker;

const DISK_MANAGER_WORKER_THREADS: usize = 4;

/// Message that a disk manager will send in response to a request.
pub enum ODiskResponse {
    BlockReady(Token)
}

//----------------------------------------------------------------------------//

pub struct InactiveDiskManager {
    inner: Arc<InnerDiskManager>
}

impl InactiveDiskManager {
    pub fn new() -> InactiveDiskManager {
        unimplemented!();
    }
    
    pub fn activate(&self, send: Box<Sender<ODiskResponse>>) -> ActiveDiskManager {
        unimplemented!();
    }
}

//----------------------------------------------------------------------------//

pub struct ActiveDiskManager {
    inner:       Arc<InnerDiskManager>,
    request_gen: TokenGenerator,
    response_id: u64
}

impl ActiveDiskManager {
    pub fn request_load() -> Token {
        unimplemented!();
    }
    
    pub fn redeem_load() {
        unimplemented!();
    }
    
    pub fn request_reserve() -> Token {
        unimplemented!();
    }
    
    pub fn redeem_reserve() {
        unimplemented!();
    }
}

//----------------------------------------------------------------------------//

struct InnerDiskManager {
    manager_send: Mutex<SyncSender<WorkerMessage>>,
    id_generator: Mutex<TokenGenerator> 
}

impl Drop for InnerDiskManager {
    fn drop(&mut self) {
        unimplemented!();
    }
}