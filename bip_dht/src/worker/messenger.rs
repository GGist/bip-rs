use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{self, SyncSender};
use std::thread;

use mio::Sender;

use worker::OneshotTask;

const OUTGOING_MESSAGE_CAPACITY: usize = 4096;

pub fn create_outgoing_messenger(socket: UdpSocket) -> SyncSender<(Vec<u8>, SocketAddr)> {
    let (send, recv) = mpsc::sync_channel::<(Vec<u8>, SocketAddr)>(OUTGOING_MESSAGE_CAPACITY);

    thread::spawn(move || {
        for (message, addr) in recv {
            send_bytes(&socket, &message[..], addr);
        }

        info!("bip_dht: Outgoing messenger received a channel hangup, exiting thread...");
    });

    send
}

fn send_bytes(socket: &UdpSocket, bytes: &[u8], addr: SocketAddr) {
    let mut bytes_sent = 0;

    while bytes_sent != bytes.len() {
        if let Ok(num_sent) = socket.send_to(&bytes[bytes_sent..], addr) {
            bytes_sent += num_sent;
        } else {
            // TODO: Maybe shut down in this case, will fail on every write...
            warn!("bip_dht: Outgoing messenger failed to write {} bytes to {}; {} bytes written \
                   before error...",
                  bytes.len(),
                  addr,
                  bytes_sent);
            break;
        }
    }
}

pub fn create_incoming_messenger(socket: UdpSocket, send: Sender<OneshotTask>) {
    thread::spawn(move || {
        let mut channel_is_open = true;

        while channel_is_open {
            let mut buffer = vec![0u8; 1500];

            match socket.recv_from(&mut buffer) {
                Ok((size, addr)) => {
                    buffer.truncate(size);
                    channel_is_open = send_message(&send, buffer, addr);
                }
                Err(_) => warn!("bip_dht: Incoming messenger failed to receive bytes..."),
            }
        }

        info!("bip_dht: Incoming messenger received a channel hangup, exiting thread...");
    });
}

fn send_message(send: &Sender<OneshotTask>, bytes: Vec<u8>, addr: SocketAddr) -> bool {
    send.send(OneshotTask::Incoming(bytes, addr)).is_ok()
}
