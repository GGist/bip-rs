use std::thread;
use std::time::Duration;
use std::sync::mpsc::{self, Sender, Receiver, TryRecvError};
use std::mem;

use bip_handshake::{BTHandshaker};

use {MockContext, MockProtocol};

#[test]
fn positive_drop() {
    // Create dummy metadata channels
    let (m_send, _): (Sender<()>, Receiver<()>) = mpsc::channel();

    // Create a context that both protocols can access
    let (context, recv) = MockContext::new();

    // Store peer ids and the info hash
    let pid_one = [0u8; 20].into();

    // Create the handshaker
    let handshaker_one = BTHandshaker::<Sender<()>, ()>::new::<MockProtocol>(m_send.clone(), "127.0.0.1:0".parse().unwrap(), pid_one, context).unwrap();

    // Drop the handshaker explicitly
    mem::drop(handshaker_one);

    // Allow the handshaker time to complete
    thread::sleep(Duration::from_millis(250));

    // Assert that our context send has disconnected
    assert_eq!(recv.try_recv(), Err(TryRecvError::Disconnected));
}