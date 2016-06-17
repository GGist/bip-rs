use std::sync::mpsc::{self, Receiver};
use std::error::Error;
use std::collections::{VecDeque, HashMap};
use std::collections::hash_map::Entry;
use std::time::Duration;

use bip_util::bt::{PeerId, InfoHash};
use bip_util::sender::Sender;
use rotor::{Scope, Time};
use rotor::mio::tcp::TcpStream;
use rotor_stream::{Protocol, Intent, Exception, Transport, Buf};
use nom::IResult;

use disk::{ActiveDiskManager, IDiskMessage, ODiskMessage};
use message::{self, MessageType};
use protocol::{PeerIdentifier, IProtocolMessage, ProtocolSender, OProtocolMessage, OProtocolMessageKind};
use protocol::machine::ProtocolContext;
use protocol::error::{ProtocolError, ProtocolErrorKind};
use piece::{OSelectorMessage, OSelectorMessageKind};
use token::Token;

// Max messages incoming to our connection from both the selection thread and disk thread.
pub const MAX_INCOMING_MESSAGES: usize = 8;

// Since we check the peer timeout lazily (because we can't have more than one timer going
// without reimplementing a timer wheel ourselves...) in the worst case we can assume a
// peer hasn't sent us a message for 1:59 (right before a timeout) + 1:30 (our own timeout,
// or, worst case time until the peer timeout is checked again) or 3 minutes and 29 seconds.
const MAX_PEER_TIMEOUT_MILLIS: u64 = 2 * 60 * 1000;
const MAX_SELF_TIMEOUT_MILLIS: u64 = (30 + 60) * 1000;

/// PeerConnection that stores information related to the messages being
/// sent and received which will propogate both out to the remote peer as
/// well as the upper, local, piece selection layer.
pub struct PeerConnection {
    id: PeerIdentifier,
    disk: ActiveDiskManager,
    recv: Receiver<IProtocolMessage>,
    state: PeerState,
    // Any writes that can immediately be executed are
    // placed inside of this queue, during a state transition
    // this queue will be checked and popped from.
    write_queue: VecDeque<(MessageType, Option<Token>)>,
    // Any writes that require the use of a block of data will
    // immediately be placed here after contacting the disk manager.
    // When the disk manager responds, the message will be taken
    // out of this queue and placed at the end of the write queue.
    block_queue: HashMap<Token, MessageType>,
    message_sent: Time,
    message_recvd: Time,
}

/// Enumeration for all states that a peer can be in in terms of messages being sent or received.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum PeerState {
    /// Read the message length; default state.
    ///
    /// Valid to transition from this state to either ReadPayload or WritePayload.
    ReadLength,
    /// Read the message length + the message itself.
    ReadPayload(usize),
    /// Wait for the disk to reserve memory for the block.
    DiskReserve(Token, usize),
    /// Write (flush) a single message to the peer.
    WritePayload,
}

impl PeerConnection {
    /// Create a new PeerConnection and returning the given Intent for the Protocol.
    fn new(id: PeerIdentifier, disk: ActiveDiskManager, recv: Receiver<IProtocolMessage>, now: Time) -> Intent<PeerConnection> {
        let connection = PeerConnection {
            id: id,
            state: PeerState::ReadLength,
            disk: disk,
            recv: recv,
            write_queue: VecDeque::new(),
            block_queue: HashMap::new(),
            message_sent: now,
            message_recvd: now,
        };

        let self_timeout = connection.self_timeout(now);
        Intent::of(connection).expect_bytes(message::MESSAGE_LENGTH_LEN_BYTES).deadline(self_timeout)
    }

    /// Returns true if the peer has exceeded it's timeout (no message received for a while).
    fn peer_timeout(&self, now: Time) -> bool {
        let max_peer_timeout = Duration::from_millis(MAX_PEER_TIMEOUT_MILLIS);

        // Since Time does not implement Sub, we convert (now - recvd > timeout) to (now < recvd + timeout)
        now < self.message_recvd + max_peer_timeout
    }

    /// Returns the timeout for ourselves at which point we will send a keep alive message.
    fn self_timeout(&self, now: Time) -> Time {
        now + Duration::from_millis(MAX_SELF_TIMEOUT_MILLIS)
    }

    /// Process the message to be written to the remote peer.
    ///
    /// Returns true if a disconnnect from the peer should be initiated.
    fn process_message(&mut self, now: Time, msg: OSelectorMessage) -> bool {
        // Check for any bugs in the selection layer sending us an invalid peer identifier
        if msg.id() != self.id {
            panic!("bip_peer: Protocol Layer Received Invalid Message ID From Selection Layer, Received: {:?} Expected: {:?}",
                   msg.id(),
                   self.id);
        }
        self.message_sent = now;

        match msg.kind() {
            OSelectorMessageKind::PeerKeepAlive => self.write_queue.push_back((MessageType::KeepAlive, None)),
            OSelectorMessageKind::PeerDisconnect => (),
            OSelectorMessageKind::PeerChoke => self.write_queue.push_back((MessageType::Choke, None)),
            OSelectorMessageKind::PeerUnChoke => self.write_queue.push_back((MessageType::UnChoke, None)),
            OSelectorMessageKind::PeerInterested => self.write_queue.push_back((MessageType::Interested, None)),
            OSelectorMessageKind::PeerNotInterested => self.write_queue.push_back((MessageType::UnInterested, None)),
            OSelectorMessageKind::PeerHave(have_msg) => self.write_queue.push_back((MessageType::Have(have_msg), None)),
            OSelectorMessageKind::PeerBitField(bfield_msg) => self.write_queue.push_back((MessageType::BitField(bfield_msg), None)),
            OSelectorMessageKind::PeerRequest(req_msg) => self.write_queue.push_back((MessageType::Request(req_msg), None)),
            OSelectorMessageKind::PeerPiece(token, piece_msg) => {
                // Sign up to receive a notification when the block associated with
                // the token has been loaded and is ready to be read from the manager
                self.disk.send(IDiskMessage::WaitBlock(token));
                self.block_queue.insert(token, MessageType::Piece(piece_msg));
            }
            OSelectorMessageKind::PeerCancel(cancel_msg) => self.write_queue.push_back((MessageType::Cancel(cancel_msg), None)),
        }

        msg.kind() == OSelectorMessageKind::PeerDisconnect
    }

    /// Process the disk event for the given token which may or may not advance our state.
    fn process_disk(&mut self, in_buffer: &mut Buf, token: Token) {
        let curr_state = self.state;

        match (self.block_queue.entry(token), curr_state) {
            (Entry::Occupied(mut occ), _) => {
                // Disk manager has loaded a block for us to write to the peer, move the message to our write_queue
                self.write_queue.push_back((occ.remove(), Some(token)));
            }
            (Entry::Vacant(_), PeerState::DiskReserve(tok, len)) if tok == token => {
                // Disk manager has reserved a block for us to write our received block to
                self.disk.redeem_reserve(tok, &in_buffer[..len]);

                in_buffer.consume(len);
                self.state = PeerState::ReadLength;
            }
            (Entry::Vacant(_), PeerState::DiskReserve(tok, len)) => unreachable!("bip_peer: Token Returned By DiskManager Was Not Expected"),
            _ => unreachable!("bip_peer: Called ProcessDisk In An Invalid State {:?}", curr_state),
        };
    }

    /// Transition our state into a disconnected state.
    fn advance_disconnect<F>(self, sel_send: F, error: ProtocolError) -> Intent<PeerConnection>
        where F: Fn(OProtocolMessage)
    {
        sel_send(OProtocolMessage::new(self.id, OProtocolMessageKind::PeerDisconnect));

        Intent::error(Box::new(error))
    }

    /// Attempts to advance our state from a read event.
    fn advance_read<F>(mut self, now: Time, in_buffer: &mut Buf, out_buffer: &mut Buf, sel_send: F) -> Intent<PeerConnection>
        where F: Fn(OProtocolMessage)
    {
        let curr_state = self.state;

        match curr_state {
            PeerState::ReadLength => {
                // Don't consume the bytes that make up the length, add that back into the expected length
                let expected_len = message::parse_message_length(&in_buffer[..]) + message::MESSAGE_LENGTH_LEN_BYTES;
                self.state = PeerState::ReadPayload(expected_len);
            }
            PeerState::ReadPayload(len) => {
                let res_opt_kind_msg = parse_kind_message(self.id, &in_buffer[..len], &self.disk);

                // For whatever message we received, propogate it up a layer (it is impossible to
                // receive a peer disconnect message off the wire, so we assume we arent propogating
                // that message)
                match res_opt_kind_msg {
                    Ok(Some(OProtocolMessageKind::PeerPiece(token, piece_msg))) => {
                        in_buffer.consume(len - piece_msg.block_length());
                        self.state = PeerState::DiskReserve(token, piece_msg.block_length());

                        // Disk manager will notify us when the memory is reserved
                        self.disk.send(IDiskMessage::WaitBlock(token));
                        sel_send(OProtocolMessage::new(self.id, OProtocolMessageKind::PeerPiece(token, piece_msg)));
                    }
                    Ok(opt_kind) => {
                        in_buffer.consume(len);
                        self.state = PeerState::ReadLength;

                        if let Some(kind) = opt_kind {
                            sel_send(OProtocolMessage::new(self.id, kind));
                        }
                    }
                    Err(prot_error) => {
                        // Early return, peer gave us an invalid message
                        return self.advance_disconnect(sel_send, prot_error);
                    }
                }
            }
            _ => unreachable!("bip_peer: Called AdvanceRead In An Invalid State {:?}", curr_state),
        }

        self.advance_write(now, out_buffer, false)
    }

    /// Attempts to advance our state to/from a write event.
    ///
    /// Since we are working with a half duplex abstraction, anytime we transition from some state back to PeerState::ReadLength,
    /// we should attempt to transition into a write state (we aggressively try to transition to a write) because that is the only
    /// time we can take control of the stream and write to the peer. The upper layer will have to make sure that it doesn't starve
    /// ourselves of reads (since there is no notion of "flow" control provided back to that layer). We may want to look at this again
    /// in the future and provide the upper layer with feedback for when writes succeeded, although it would be preferrable to not do so.
    fn advance_write(mut self, now: Time, mut out_buffer: &mut Buf, bytes_flushed: bool) -> Intent<PeerConnection> {
        // First, check if this was called from a bytes flushed event
        if bytes_flushed {
            // "Reset" our state
            self.state = PeerState::ReadLength;
        }

        // Next, check if we can transition to/back to a write event
        if !self.write_queue.is_empty() && self.state == PeerState::ReadLength {
            let (msg, opt_token) = self.write_queue.pop_front().unwrap();

            // We can write out this message, and an optional payload from disk
            msg.write_bytes(&mut out_buffer).unwrap();
            if let Some(token) = opt_token {
                out_buffer.extend(self.disk.redeem_load(token));
            }

            self.state = PeerState::WritePayload;
        }

        // Figure our what intent we should return based on our CURRENT state, even if unchanged
        let self_timeout = self.self_timeout(now);
        match self.state {
            PeerState::ReadLength => Intent::of(self).expect_bytes(message::MESSAGE_LENGTH_LEN_BYTES).deadline(self_timeout),
            PeerState::ReadPayload(len) => Intent::of(self).expect_bytes(len).deadline(self_timeout),
            PeerState::DiskReserve(..) => Intent::of(self).sleep().deadline(self_timeout),
            PeerState::WritePayload => Intent::of(self).expect_flush().deadline(self_timeout),
        }
    }
}

/// Attempt to parse the peer message as an OProtocolMessageKind.
fn parse_kind_message(id: PeerIdentifier, bytes: &[u8], disk: &ActiveDiskManager) -> Result<Option<OProtocolMessageKind>, ProtocolError> {
    match MessageType::from_bytes(bytes) {
        IResult::Done(_, msg_type) => Ok(map_message_type(msg_type, disk)),
        IResult::Error(_) |
        IResult::Incomplete(_) => Err(ProtocolError::new(id, ProtocolErrorKind::InvalidMessage)),
    }
}

/// Maps a message type as an OProtocolMessageKind.
fn map_message_type(msg_type: MessageType, disk: &ActiveDiskManager) -> Option<OProtocolMessageKind> {
    match msg_type {
        MessageType::KeepAlive => None,
        MessageType::Choke => Some(OProtocolMessageKind::PeerChoke),
        MessageType::UnChoke => Some(OProtocolMessageKind::PeerUnChoke),
        MessageType::Interested => Some(OProtocolMessageKind::PeerInterested),
        MessageType::UnInterested => Some(OProtocolMessageKind::PeerUnInterested),
        MessageType::Have(msg) => Some(OProtocolMessageKind::PeerHave(msg)),
        MessageType::BitField(msg) => Some(OProtocolMessageKind::PeerBitField(msg)),
        MessageType::Request(msg) => Some(OProtocolMessageKind::PeerRequest(msg)),
        MessageType::Piece(msg) => Some(OProtocolMessageKind::PeerPiece(disk.gen_request_token(), msg)),
        MessageType::Cancel(msg) => Some(OProtocolMessageKind::PeerCancel(msg)),
        MessageType::Extension(_) => unimplemented!(),
    }
}

impl Protocol for PeerConnection {
    type Context = ProtocolContext;
    type Socket = TcpStream;
    type Seed = (PeerId, InfoHash);

    fn create((pid, hash): Self::Seed, sock: &mut Self::Socket, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        let id = PeerIdentifier::new(sock.peer_addr().unwrap(), pid);
        let (send, recv) = mpsc::sync_channel(MAX_INCOMING_MESSAGES);
        let prot_send = ProtocolSender::new(send, scope.notifier());

        let disk = scope.register_disk(Box::new(prot_send.clone()));
        scope.send_selector(OProtocolMessage::new(id, OProtocolMessageKind::PeerConnect(Box::new(prot_send), hash)));

        PeerConnection::new(id, disk, recv, scope.now())
    }

    fn bytes_read(self, transport: &mut Transport<Self::Socket>, end: usize, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        let now = scope.now();
        let id = self.id;

        if self.peer_timeout(now) {
            self.advance_disconnect(|msg| scope.send_selector(msg), ProtocolError::new(id, ProtocolErrorKind::RemoteTimeout))
        } else {
            let (input, output) = transport.buffers();

            self.advance_read(now, input, output, |msg| scope.send_selector(msg))
        }
    }

    fn bytes_flushed(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        let now = scope.now();
        let id = self.id;

        if self.peer_timeout(now) {
            self.advance_disconnect(|msg| scope.send_selector(msg), ProtocolError::new(id, ProtocolErrorKind::RemoteTimeout))
        } else {
            self.advance_write(now, transport.output(), true)
        }
    }

    fn timeout(mut self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        let now = scope.now();
        let id = self.id;

        if self.peer_timeout(now) {
            self.advance_disconnect(|msg| scope.send_selector(msg), ProtocolError::new(id, ProtocolErrorKind::RemoteTimeout))
        } else {
            // All we can do here is push a keep alive message on to our queue since we can't necessarily transition to a write payload state
            // for example, if we are still waiting on the disk manager. Also, we will update our message_sent whenever we push to the write
            // queue to make it easy for us to know what we mean when we talk about our write timeout.
            let id = self.id;
            self.process_message(now, OSelectorMessage::new(id, OSelectorMessageKind::PeerKeepAlive));

            self.advance_write(now, transport.output(), false)
        }
    }

    fn exception(self, _transport: &mut Transport<Self::Socket>, reason: Exception, _scope: &mut Scope<Self::Context>) -> Intent<Self> {
        let id = self.id;

        self.advance_disconnect(|msg| _scope.send_selector(msg), ProtocolError::new(id, ProtocolErrorKind::RemoteDisconnect))
    }

    fn fatal(self, reason: Exception, scope: &mut Scope<Self::Context>) -> Option<Box<Error>> {
        let id = self.id;
        let _ = self.advance_disconnect(|msg| scope.send_selector(msg), ProtocolError::new(id, ProtocolErrorKind::RemoteError));

        None
    }

    fn wakeup(mut self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {
        let now = scope.now();
        let id = self.id;

        if self.peer_timeout(now) {
            self.advance_disconnect(|msg| scope.send_selector(msg), ProtocolError::new(id, ProtocolErrorKind::RemoteTimeout))
        } else {
            while let Ok(msg) = self.recv.try_recv() {
                match msg {
                    IProtocolMessage::DiskManager(ODiskMessage::BlockReady(token)) => {
                        self.process_disk(transport.input(), token);
                    }
                    IProtocolMessage::PieceManager(sel_msg) => {
                        // If the selection layer sent us a disconnect message, handle it here
                        if self.process_message(now, sel_msg) {
                            return self.advance_disconnect(|msg| scope.send_selector(msg),
                                                           ProtocolError::new(id, ProtocolErrorKind::RemoteDisconnect));
                        }
                    }
                }
            }

            self.advance_write(now, transport.output(), false)
        }
    }
}
