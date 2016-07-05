extern crate bip_handshake;
extern crate rotor_stream;

use bip_handshake::{BTSeed};

mod test_tcp_handshake_connect;

struct MockContext {
    send: Sender<BTSeed>
}