use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::net::SocketAddr;
use std::io;
use std::thread;
use std::marker::PhantomData;

use bip_util::bt::{InfoHash, PeerId};
use bip_util::send::TrySender;
use rotor::{Loop, Config, Response};

use bittorrent::seed::InitiateSeed;
use bittorrent::handshake::protocol::PeerHandshake;
use bittorrent::handshake::context;
use bittorrent::machine::accept::Accept;
use bittorrent::machine::initiate::{Initiate, InitiateSender, InitiateMessage};
use handshaker::Handshaker;
use peer_protocol::PeerProtocol;
use try_bind::TryBind;
use local_address::LocalAddress;

const BTP_10_PROTOCOL: &'static str = "BitTorrent protocol";

const MAXIMUM_ACTIVE_CONNECTIONS: usize = 8192;

/// Bittorrent handshaker that can compose with a `PeerProtocol`.
pub struct BTHandshaker<S, M> {
    meta_send: S,
    peer_send: InitiateSender<Sender<InitiateMessage>>,
    interest: Arc<RwLock<HashSet<InfoHash>>>,
    pid: PeerId,
    port: u16,
    ref_check: Arc<AtomicUsize>,
    _metadata: PhantomData<M>,
}

impl<S, M> BTHandshaker<S, M> {
    /// Create a new BTHandshaker operating over the standard BTP_10 protocol.
    ///
    /// See `BTHandshaker::with_protocol` for more details.
    pub fn new<P>(metadata: S, listen: SocketAddr, pid: PeerId, context: P::Context) -> io::Result<BTHandshaker<S, M>>
        where P: PeerProtocol + 'static,
              P::Context: Send + 'static
    {
        BTHandshaker::with_protocol::<P>(metadata, listen, pid, context, BTP_10_PROTOCOL)
    }

    /// Create a new BTHandshaker operating over the given protocol. Metadata will be forwarded to the given channel, connections
    /// will be accepted on the given listen address, and the specified PeerProtocol will have access to the supplied context.
    ///
    /// Panics if the protocol specified is longer than 255 bytes.
    pub fn with_protocol<P>(metadata: S,
                            listen: SocketAddr,
                            pid: PeerId,
                            context: P::Context,
                            protocol: &'static str)
                            -> io::Result<BTHandshaker<S, M>>
        where P: PeerProtocol + 'static,
              P::Context: Send + 'static
    {
        if protocol.len() > 255 {
            panic!("bip_handshake: Protocol With Length Greater Than 255 Detected")
        }
        let interest = Arc::new(RwLock::new(HashSet::new()));
        let (peer_send, port) = try!(spawn_state_machine::<P>(listen, pid, context, interest.clone(), protocol));

        Ok(BTHandshaker {
            meta_send: metadata,
            peer_send: peer_send,
            interest: interest,
            pid: pid,
            port: port,
            ref_check: Arc::new(AtomicUsize::new(1)),
            _metadata: PhantomData,
        })
    }

    /// Register interest for the given InfoHash allowing connections for the given InfoHash to succeed.
    /// Connections already in the handshaking process may not be affected by this call.
    ///
    /// Connections already in the handshaking process MAY NOT be affected by this call.
    /// If a peer connection with an inactive InfoHash is spun up, you should handle it in the PeerProtocol.
    ///
    /// This is a blocking operation.
    pub fn register(&self, hash: InfoHash) {
        self.interest.write().unwrap().insert(hash);
    }

    /// Deregister interest for the given InfoHash causing connections for the given InfoHash to fail.
    ///
    /// See `BTHandshaker::register` for more information.
    pub fn deregister(&self, hash: InfoHash) {
        self.interest.write().unwrap().remove(&hash);
    }
}

impl<S, M> Drop for BTHandshaker<S, M> {
    fn drop(&mut self) {
        // Returns the previous stored value, subtract 1 to get the currently stored value after the fetch_sub
        let active_refs = self.ref_check.fetch_sub(1, Ordering::SeqCst) - 1;

        if active_refs == 0 {
            // TODO: Use a SplitSender here in the future to guarantee there is space
            assert!(self.peer_send.try_send(InitiateMessage::Shutdown).is_none());
        }
    }
}

impl<S, M> Handshaker for BTHandshaker<S, M>
    where S: TrySender<M>,
          M: Send
{
    type MetadataEnvelope = M;

    fn id(&self) -> PeerId {
        self.pid
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn connect(&mut self, expected: Option<PeerId>, hash: InfoHash, addr: SocketAddr) {
        // If this becomes a performance problem, we can move this to the state machine,
        // however, the benefit for checking here is that we can check before setting
        // up the actual transport, otherwise, that transport may get setup and immediately
        // torn down.
        if self.interest.read().expect("bip_handshake: Client Failed To Read Interest").contains(&hash) {
            let init_seed = match expected {
                Some(pid) => InitiateSeed::expect_pid(addr, hash, pid),
                None => InitiateSeed::new(addr, hash),
            };

            if let Some(_) = self.peer_send.try_send(InitiateMessage::Initiate(init_seed)) {
                // TODO: Add logging?
            }
        }
    }

    fn metadata(&mut self, data: Self::MetadataEnvelope) {
        if let Some(_) = self.meta_send.try_send(data) {
            // TODO: Add logging?
        }
    }
}

impl<S, M> Clone for BTHandshaker<S, M>
    where S: Clone
{
    fn clone(&self) -> BTHandshaker<S, M> {
        self.ref_check.fetch_add(1, Ordering::SeqCst);

        BTHandshaker {
            meta_send: self.meta_send.clone(),
            peer_send: self.peer_send.clone(),
            interest: self.interest.clone(),
            pid: self.pid,
            port: self.port,
            ref_check: self.ref_check.clone(),
            _metadata: PhantomData,
        }
    }
}

fn spawn_state_machine<P>(listen: SocketAddr,
                          pid: PeerId,
                          context: P::Context,
                          interest: Arc<RwLock<HashSet<InfoHash>>>,
                          protocol: &'static str)
                          -> io::Result<(InitiateSender<Sender<InitiateMessage>>, u16)>
    where P: PeerProtocol + 'static,
          P::Context: Send + 'static
{
    let context = context::peer_context_new(protocol, pid, interest, context);

    let mut config = Config::new();
    config.slab_capacity(MAXIMUM_ACTIVE_CONNECTIONS);

    let mut eloop: Loop<Accept<Initiate<PeerHandshake<P::Socket, P::Context>, P::Protocol>, P::Listener>> = try!(Loop::new(&config));

    // Startup Listener State Machine
    let listener = try!(P::Listener::try_bind(listen));
    // Grabbing this after the bind so that we can resolve '0' port numbers
    let port = try!(listener.local_address()).port();
    eloop.add_machine_with(|early| Accept::new(listener, early))
        .expect("bip_handshake: Failed To Start TcpListener State Machine");

    // Startup Connection Initiation State Machine
    let (send, recv) = mpsc::channel();
    let mut peer_send = None;
    eloop.add_machine_with(|early| {
            peer_send = Some(InitiateSender::new(send, early.notifier()));

            Response::ok(Accept::Connection(Initiate::Recv(recv)))
        })
        .expect("bip_handshake: Failed To Start Connection Initiation State Machine");

    thread::spawn(move || {
        eloop.run(context).expect("bip_handshake: State Machines Failed Unexpectedly");
    });

    Ok((peer_send.unwrap(), port))
}
