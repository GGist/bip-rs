use std::marker::PhantomData;
use std::sync::mpsc::Sender;
use std::io::{self, Write};
use std::error::Error;
use std::time::Duration;

use bip_util::bt::{PeerId, InfoHash};
use rotor::{Scope, Time};
use rotor_stream::{StreamSocket, Intent, Transport, Buf, Protocol, Exception};

use bittorrent::handshake::{HandshakeSeed, HandshakeState, InitiateState, CompleteState};
use bittorrent::handshake::context::{self, BTContext};
use bittorrent::handshake::parse;
use bittorrent::seed::BTSeed;

const PEER_READ_TIMEOUT_MILLIS: u64 = 5000;

pub struct PeerHandshake<T, C> {
    send: Sender<BTSeed>,
    next_state: HandshakeState,
    _transport: PhantomData<T>,
    _context: PhantomData<C>,
}

impl<T, C> PeerHandshake<T, C>
    where T: StreamSocket
{
    pub fn new(seed: HandshakeSeed, send: Sender<BTSeed>) -> PeerHandshake<T, C> {
        let next_state = match seed {
            HandshakeSeed::Initiate(init_seed) => HandshakeState::Initiate(InitiateState::WriteMessage(init_seed.0), init_seed.1),
            HandshakeSeed::Complete(comp_seed) => HandshakeState::Complete(CompleteState::ReadLength(comp_seed.0)),
        };

        PeerHandshake {
            next_state: next_state,
            send: send,
            _transport: PhantomData,
            _context: PhantomData,
        }
    }

    pub fn advance(self, now: Time, context: &BTContext<C>, read: &mut Buf, write: &mut Buf) -> Intent<PeerHandshake<T, C>> {
        let state_type = self.next_state;

        match state_type {
            HandshakeState::Initiate(next, exp_pid) => advance_initiate(self, now, context, read, write, next, exp_pid),
            HandshakeState::Complete(next) => advance_complete(self, now, context, read, write, next),
        }
    }
}

fn advance_initiate<C, T>(mut prot: PeerHandshake<T, C>,
                          now: Time,
                          context: &BTContext<C>,
                          read: &mut Buf,
                          write: &mut Buf,
                          next: InitiateState,
                          exp_pid: Option<PeerId>)
                          -> Intent<PeerHandshake<T, C>> {
    match next {
        InitiateState::WriteMessage(partial_seed) => {
            let res_write =
                write_handshake(write, context::peer_context_protocol(context), partial_seed.hash(), context::peer_context_pid(context));

            if let Err(write_err) = res_write {
                Intent::error(Box::new(write_err))
            } else {
                prot.next_state = HandshakeState::Initiate(InitiateState::ReadLength(partial_seed), exp_pid);

                Intent::of(prot).expect_flush()
            }
        }
        InitiateState::ReadLength(partial_seed) => {
            prot.next_state = HandshakeState::Initiate(InitiateState::ReadMessage(partial_seed), exp_pid);

            Intent::of(prot).expect_bytes(1).deadline(now + Duration::from_millis(PEER_READ_TIMEOUT_MILLIS))
        }
        InitiateState::ReadMessage(partial_seed) => {
            let prot_len = read[0] as usize;
            let our_prot_len = context::peer_context_protocol(context).len();

            if prot_len != our_prot_len {
                Intent::error(Box::new(io::Error::new(io::ErrorKind::ConnectionAborted, "Protocol Length Mismatch")))
            } else {
                prot.next_state = HandshakeState::Initiate(InitiateState::Done(partial_seed), exp_pid);

                Intent::of(prot).expect_bytes(1 + our_prot_len + 48).deadline(now + Duration::from_millis(PEER_READ_TIMEOUT_MILLIS))
            }
        }
        InitiateState::Done(partial_seed) => {
            // Since we initiated the connection, we already know we are interested in the hash, no point on locking (we did this in the client)
            let res_read = read_handshake(&read[..],
                                          context::peer_context_protocol(context),
                                          exp_pid,
                                          |_| true);

            let read_length = read.len();
            read.consume(read_length);

            // However, we do want to make sure that the info hash they give us matches what we gave them
            match res_read {
                Ok((hash, pid)) if hash == partial_seed.hash() => {
                    prot.send
                        .send(partial_seed.found(pid))
                        .expect("bip_handshake: Failed To Send Seed From Finished Handshaker");

                    Intent::of(prot).sleep()
                },
                Ok(_) => Intent::error(Box::new(io::Error::new(io::ErrorKind::ConnectionAborted, "InfoHash Mismatch From Peer"))),
                Err(err) => Intent::error(Box::new(err)),
            }
        }
    }
}

fn advance_complete<C, T>(mut prot: PeerHandshake<T, C>,
                          now: Time,
                          context: &BTContext<C>,
                          read: &mut Buf,
                          write: &mut Buf,
                          next: CompleteState)
                          -> Intent<PeerHandshake<T, C>> {
    match next {
        CompleteState::ReadLength(empty_seed) => {
            prot.next_state = HandshakeState::Complete(CompleteState::ReadMessage(empty_seed));

            Intent::of(prot).expect_bytes(1).deadline(now + Duration::from_millis(PEER_READ_TIMEOUT_MILLIS))
        }
        CompleteState::ReadMessage(empty_seed) => {
            let prot_len = read[0] as usize;
            let our_prot_len = context::peer_context_protocol(context).len();

            if prot_len != our_prot_len {
                Intent::error(Box::new(io::Error::new(io::ErrorKind::ConnectionAborted, "Protocol Length Mismatch")))
            } else {
                prot.next_state = HandshakeState::Complete(CompleteState::WriteMessage(empty_seed));

                Intent::of(prot).expect_bytes(1 + our_prot_len + 48).deadline(now + Duration::from_millis(PEER_READ_TIMEOUT_MILLIS))
            }
        }
        CompleteState::WriteMessage(empty_seed) => {
            let res_read = read_handshake(&read[..],
                                          context::peer_context_protocol(context),
                                          None,
                                          |hash| context::peer_context_interest(context, hash));

            let read_length = read.len();
            read.consume(read_length);

            let bt_seed = match res_read {
                Ok((hash, pid)) => empty_seed.found(hash).found(pid),
                Err(err) => return Intent::error(Box::new(err)),
            };

            let res_write =
                write_handshake(write, context::peer_context_protocol(context), bt_seed.hash(), context::peer_context_pid(context));

            if let Err(write_err) = res_write {
                Intent::error(Box::new(write_err))
            } else {
                prot.next_state = HandshakeState::Complete(CompleteState::Done(bt_seed));

                Intent::of(prot).expect_flush()
            }
        }
        CompleteState::Done(bt_seed) => {
            prot.send
                .send(bt_seed)
                .expect("bip_handshake: Failed To Send Seed From Finished Handshaker");

            Intent::of(prot).sleep()
        }
    }
}

fn write_handshake<W>(mut writer: W, protocol: &'static str, hash: InfoHash, pid: PeerId) -> io::Result<()>
    where W: Write
{
    try!(writer.write_all(&[protocol.len() as u8]));
    try!(writer.write_all(protocol.as_bytes()));
    try!(writer.write_all(&[0u8; 8]));
    try!(writer.write_all(hash.as_ref()));
    try!(writer.write_all(pid.as_ref()));

    Ok(())
}

fn read_handshake<F>(bytes: &[u8],
                     expected_protocol: &'static str,
                     expected_pid: Option<PeerId>,
                     hash_interest: F)
                     -> io::Result<(InfoHash, PeerId)>
    where F: Fn(&InfoHash) -> bool
{
    parse::parse_remote_handshake(bytes, expected_pid, expected_protocol).and_then(|(hash, pid)| {
        if hash_interest(&hash) {
            Ok((hash, pid))
        } else {
            Err(io::Error::new(io::ErrorKind::ConnectionAborted, "No Interest For Handshake InfoHash"))
        }
    })
}

impl<T, C> Protocol for PeerHandshake<T, C>
    where T: StreamSocket
{
    type Context = BTContext<C>;
    type Socket = T;
    type Seed = (HandshakeSeed, Sender<BTSeed>);

    fn create((handshake_seed, peer_seed): Self::Seed, _sock: &mut Self::Socket, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        if scope.notifier().wakeup().is_ok() {
            Intent::of(PeerHandshake::new(handshake_seed, peer_seed)).sleep()
        } else {
            Intent::error(Box::new(io::Error::new(io::ErrorKind::Other, "Failed To Wakeup New Handshaker")))
        }
    }

    fn bytes_read(self, transport: &mut Transport<Self::Socket>, _end: usize, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        let (read, write) = transport.buffers();
        let now = scope.now();

        self.advance(now, &**scope, read, write)
    }

    fn bytes_flushed(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        let (read, write) = transport.buffers();
        let now = scope.now();

        self.advance(now, &**scope, read, write)
    }

    fn timeout(self, _transport: &mut Transport<Self::Socket>, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        Intent::error(Box::new(io::Error::new(io::ErrorKind::TimedOut, "Remote Peer Handshake Timed Out")))
    }

    fn exception(self, _transport: &mut Transport<Self::Socket>, _reason: Exception, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        Intent::error(Box::new(io::Error::new(io::ErrorKind::ConnectionAborted, "Remote Peer Aborted The Handshake")))
    }

    fn fatal(self, _reason: Exception, _scope: &mut Scope<Self::Context>) -> Option<Box<Error>> {
        None
    }

    fn wakeup(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        // We only trigger this after the initial create because we don't have access to the transport in the create method.
        let (read, write) = transport.buffers();
        let now = scope.now();

        self.advance(now, &**scope, read, write)
    }
}
