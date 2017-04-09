use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use disk::fs::FileSystem;
use disk::{IDiskMessage, ODiskMessage};

use futures::sync::mpsc::Sender;
use futures_cpupool::CpuPool;

pub fn execute_on_pool<F>(pool: &CpuPool, done: Arc<AtomicUsize>, res_send: Sender<ODiskMessage>, fs: Arc<F>, msg: IDiskMessage)
    where F: FileSystem {
    /*match msg {
        IDiskMessage::AddTorrent(metainfo) => ,
        IDIskMessage::RemoveTorrent(info_hash) => ,
        
    }*/
    unimplemented!()
}