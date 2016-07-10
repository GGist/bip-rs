#![allow(unused)]

use std::sync::{Arc, Mutex};
use std::sync::mpsc::SyncSender;

use bip_util::send::TrySender;

use disk::worker::WorkerMessage;
use registration::LayerRegistration;
use token::{Token, TokenGenerator};

mod worker;

const DISK_MANAGER_WORKER_THREADS: usize = 4;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum IDiskMessage {
    WaitBlock(Token),
    LoadBlock(Token),
    ReserveBlock(Token),
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum ODiskMessage {
    BlockReady(Token),
}

// ----------------------------------------------------------------------------//

pub struct InactiveDiskManager; //{
    //inner: Arc<InnerDiskManager>,
//}

impl InactiveDiskManager {
    pub fn new() -> InactiveDiskManager {
        InactiveDiskManager
    }
}

impl LayerRegistration<ODiskMessage, IDiskMessage> for InactiveDiskManager {
    type SS2 = ActiveDiskManager;

    fn register(&self, send: Box<TrySender<ODiskMessage>>) -> ActiveDiskManager {
        ActiveDiskManager::new()
    }
}

// ----------------------------------------------------------------------------//

// Selectors will have to call an InfoHash setup and teardown function to attach
// or detach disk fds to the disk manager on a per InfoHash basis.

// To load a block, a selector will give all of the pertinent information to the
// disk manager, as well as the InfoHash.

// To reserve a block, a protocol will reserve the space and write to it.

// A selector will receive the token associated with that reserved (written to)
// block and essentially claim it, telling the disk manager that it belongs
// to so and so InfoHash.

pub struct ActiveDiskManager {
    // inner: Arc<InnerDiskManager>,
    // request_gen: TokenGenerator,
    id: u64,
}

impl ActiveDiskManager {
    pub fn new() -> ActiveDiskManager {
        ActiveDiskManager { id: 5 }
    }

    pub fn wait_load(&self, token: Token) {}

    pub fn redeem_load(&self, token: Token) -> &[u8] {
        unimplemented!();
    }

    pub fn redeem_reserve(&self, token: Token, block: &[u8]) {
        unimplemented!();
    }

    pub fn gen_request_token(&self) -> Token {
        unimplemented!();
    }
}

impl TrySender<IDiskMessage> for ActiveDiskManager {
    fn try_send(&self, data: IDiskMessage) -> Option<IDiskMessage> {
        unimplemented!();
    }
}

// ----------------------------------------------------------------------------//

struct InnerDiskManager {
    manager_send: Mutex<SyncSender<WorkerMessage>>,
    id_generator: Mutex<TokenGenerator>,
}

impl Drop for InnerDiskManager {
    fn drop(&mut self) {
        unimplemented!();
    }
}
