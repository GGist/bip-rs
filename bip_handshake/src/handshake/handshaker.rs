use std::cmp;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use crate::discovery::DiscoveryInfo;
use crate::filter::filters::Filters;
use crate::filter::{HandshakeFilter, HandshakeFilters};
use crate::handshake::config::HandshakerConfig;
use crate::handshake::handler;
use crate::handshake::handler::handshaker;
use crate::handshake::handler::initiator;
use crate::handshake::handler::listener::ListenerHandler;
use crate::handshake::handler::timer::HandshakeTimer;
use crate::local_addr::LocalAddr;
use crate::message::complete::CompleteMessage;
use crate::message::extensions::Extensions;
use crate::message::initiate::InitiateMessage;
use crate::transport::Transport;

use bip_util::bt::PeerId;
use bip_util::convert;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::sync::mpsc::{self, Receiver, SendError, Sender};
use futures::{Poll, StartSend};
use rand::RngCore;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};

/// Build configuration for `Handshaker` object creation.
#[derive(Copy, Clone)]
pub struct HandshakerBuilder {
    bind: SocketAddr,
    port: u16,
    pid: PeerId,
    ext: Extensions,
    config: HandshakerConfig,
}

impl HandshakerBuilder {
    /// Create a new `HandshakerBuilder`.
    pub fn new() -> HandshakerBuilder {
        let default_v4_addr = Ipv4Addr::new(0, 0, 0, 0);
        let default_v4_port = 0;

        let default_sock_addr = SocketAddr::V4(SocketAddrV4::new(default_v4_addr, default_v4_port));

        let seed = rand::thread_rng().next_u32();
        let default_peer_id = PeerId::from_bytes(&convert::four_bytes_to_array(seed));

        HandshakerBuilder {
            bind: default_sock_addr,
            port: default_v4_port,
            pid: default_peer_id,
            ext: Extensions::new(),
            config: HandshakerConfig::default(),
        }
    }

    /// Address that the host will listen on.
    ///
    /// Defaults to IN_ADDR_ANY using port 0 (any free port).
    pub fn with_bind_addr(&mut self, addr: SocketAddr) -> &mut HandshakerBuilder {
        self.bind = addr;

        self
    }

    /// Port that external peers should connect on.
    ///
    /// Defaults to the port that is being listened on (will only work if the
    /// host is not natted).
    pub fn with_open_port(&mut self, port: u16) -> &mut HandshakerBuilder {
        self.port = port;

        self
    }

    /// Peer id that will be advertised when handshaking with other peers.
    ///
    /// Defaults to a random SHA-1 hash; official clients should use an encoding scheme.
    ///
    /// See http://www.bittorrent.org/beps/bep_0020.html.
    pub fn with_peer_id(&mut self, peer_id: PeerId) -> &mut HandshakerBuilder {
        self.pid = peer_id;

        self
    }

    /// Extensions supported by our client, advertised to the peer when handshaking.
    pub fn with_extensions(&mut self, ext: Extensions) -> &mut HandshakerBuilder {
        self.ext = ext;

        self
    }

    /// Configuration that will be used to alter the internal behavior of handshaking.
    ///
    /// This will typically not need to be set unless you know what you are doing.
    pub fn with_config(&mut self, config: HandshakerConfig) -> &mut HandshakerBuilder {
        self.config = config;

        self
    }

    /// Build a `Handshaker` over the given `Transport` with a `Remote` instance.
    pub fn build<T>(&self, transport: T, handle: Handle) -> io::Result<Handshaker<T::Socket>>
    where
        T: Transport + 'static,
    {
        Handshaker::with_builder(self, transport, handle)
    }
}

//----------------------------------------------------------------------------------//

/// Handshaker which is both `Stream` and `Sink`.
pub struct Handshaker<S> {
    sink: HandshakerSink,
    stream: HandshakerStream<S>,
}

impl<S> Handshaker<S> {
    /// Splits the `Handshaker` into its parts.
    ///
    /// This is an enhanced version of `Stream::split` in that the returned `Sink` implements
    /// `DiscoveryInfo` so it can be cloned and passed in to different peer discovery services.
    pub fn into_parts(self) -> (HandshakerSink, HandshakerStream<S>) {
        (self.sink, self.stream)
    }
}

impl<S> DiscoveryInfo for Handshaker<S> {
    fn port(&self) -> u16 {
        self.sink.port()
    }

    fn peer_id(&self) -> PeerId {
        self.sink.peer_id()
    }
}

impl<S> Handshaker<S>
where
    S: AsyncRead + AsyncWrite + 'static,
{
    fn with_builder<T>(builder: &HandshakerBuilder, transport: T, handle: Handle) -> io::Result<Handshaker<T::Socket>>
    where
        T: Transport<Socket = S> + 'static,
    {
        let listener = transport.listen(&builder.bind, &handle)?;

        // Resolve our "real" public port
        let open_port = if builder.port == 0 {
            listener.local_addr()?.port()
        } else {
            builder.port
        };

        let config = builder.config;
        let (addr_send, addr_recv) = mpsc::channel(config.sink_buffer_size());
        let (hand_send, hand_recv) = mpsc::channel(config.wait_buffer_size());
        let (sock_send, sock_recv) = mpsc::channel(config.done_buffer_size());

        let filters = Filters::new();
        let (handshake_timer, initiate_timer) = configured_handshake_timers(config.handshake_timeout(), config.connect_timeout());

        // Hook up our pipeline of handlers which will take some connection info, process it, and forward it
        handler::loop_handler(
            addr_recv,
            initiator::initiator_handler,
            hand_send.clone(),
            (transport, filters.clone(), handle.clone(), initiate_timer),
            &handle,
        );
        handler::loop_handler(listener, ListenerHandler::new, hand_send, filters.clone(), &handle);
        handler::loop_handler(
            hand_recv.map(Result::Ok).buffer_unordered(100),
            handshaker::execute_handshake,
            sock_send,
            (builder.ext, builder.pid, filters.clone(), handshake_timer),
            &handle,
        );

        let sink = HandshakerSink::new(addr_send, open_port, builder.pid, filters);
        let stream = HandshakerStream::new(sock_recv);

        Ok(Handshaker { sink, stream })
    }
}

/// Configure a timer wheel and create a `HandshakeTimer`.
fn configured_handshake_timers(duration_one: Duration, duration_two: Duration) -> (HandshakeTimer, HandshakeTimer) {
    let timer = tokio_timer::wheel()
        .num_slots(64)
        .max_timeout(cmp::max(duration_one, duration_two))
        .build();

    (
        HandshakeTimer::new(timer.clone(), duration_one),
        HandshakeTimer::new(timer, duration_two),
    )
}

impl<S> Sink for Handshaker<S> {
    type SinkItem = InitiateMessage;
    type SinkError = SendError<InitiateMessage>;

    fn start_send(&mut self, item: InitiateMessage) -> StartSend<InitiateMessage, SendError<InitiateMessage>> {
        self.sink.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<InitiateMessage>> {
        self.sink.poll_complete()
    }
}

impl<S> Stream for Handshaker<S> {
    type Item = CompleteMessage<S>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<CompleteMessage<S>>, ()> {
        self.stream.poll()
    }
}

impl<S> HandshakeFilters for Handshaker<S> {
    fn add_filter<F>(&self, filter: F)
    where
        F: HandshakeFilter + PartialEq + Eq + Send + Sync + 'static,
    {
        self.sink.add_filter(filter);
    }

    fn remove_filter<F>(&self, filter: F)
    where
        F: HandshakeFilter + PartialEq + Eq + Send + Sync + 'static,
    {
        self.sink.remove_filter(filter);
    }

    fn clear_filters(&self) {
        self.sink.clear_filters();
    }
}

//----------------------------------------------------------------------------------//

/// `Sink` portion of the `Handshaker` for initiating handshakes.
#[derive(Clone)]
pub struct HandshakerSink {
    send: Sender<InitiateMessage>,
    port: u16,
    pid: PeerId,
    filters: Filters,
}

impl HandshakerSink {
    fn new(send: Sender<InitiateMessage>, port: u16, pid: PeerId, filters: Filters) -> HandshakerSink {
        HandshakerSink { send, port, pid, filters }
    }
}

impl DiscoveryInfo for HandshakerSink {
    fn port(&self) -> u16 {
        self.port
    }

    fn peer_id(&self) -> PeerId {
        self.pid
    }
}

impl Sink for HandshakerSink {
    type SinkItem = InitiateMessage;
    type SinkError = SendError<InitiateMessage>;

    fn start_send(&mut self, item: InitiateMessage) -> StartSend<InitiateMessage, SendError<InitiateMessage>> {
        self.send.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<InitiateMessage>> {
        self.send.poll_complete()
    }
}

impl HandshakeFilters for HandshakerSink {
    fn add_filter<F>(&self, filter: F)
    where
        F: HandshakeFilter + PartialEq + Eq + Send + Sync + 'static,
    {
        self.filters.add_filter(filter);
    }

    fn remove_filter<F>(&self, filter: F)
    where
        F: HandshakeFilter + PartialEq + Eq + Send + Sync + 'static,
    {
        self.filters.remove_filter(filter);
    }

    fn clear_filters(&self) {
        self.filters.clear_filters();
    }
}

//----------------------------------------------------------------------------------//

/// `Stream` portion of the `Handshaker` for completed handshakes.
pub struct HandshakerStream<S> {
    recv: Receiver<CompleteMessage<S>>,
}

impl<S> HandshakerStream<S> {
    fn new(recv: Receiver<CompleteMessage<S>>) -> HandshakerStream<S> {
        HandshakerStream { recv }
    }
}

impl<S> Stream for HandshakerStream<S> {
    type Item = CompleteMessage<S>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<CompleteMessage<S>>, ()> {
        self.recv.poll()
    }
}
