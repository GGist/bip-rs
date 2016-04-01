use std::io::{self};
use std::net::{SocketAddr};

use umio::external::{Sender};

use server::dispatcher::{DispatchMessage};
use server::handler::{ServerHandler};

mod dispatcher;
pub mod handler;

/// Tracker server that executes responses asynchronously.
///
/// Server will shutdown on drop.
pub struct TrackerServer {
    send: Sender<DispatchMessage>
}

impl TrackerServer {
    /// Run a new TrackerServer.
    pub fn run<H>(bind: SocketAddr, handler: H) -> io::Result<TrackerServer>
        where H: ServerHandler + 'static {
        dispatcher::create_dispatcher(bind, handler).map(|send| {
            TrackerServer{ send: send }
        })
    }
}

impl Drop for TrackerServer {
    fn drop(&mut self) {
        self.send.send(DispatchMessage::Shutdown)
            .expect("bip_utracker: TrackerServer Failed To Send Shutdown Message");
    }
}