pub struct Handshaker<T> where T: Transport {
    sink:   HandshakerSink,
    stream: HandshakerStream<T>
}

impl<T> BTHandshaker<T> {
    pub fn new() -> io::Result<BTHandshaker<T>> {

    }

    pub fn with_addr(addr: &SocketAddr) -> io::Result<BTHandshaker<T>> {

    }
}

//----------------------------------------------------------------------------------//

pub struct HandshakerSink {
    send: Sender<SocketAddr>
}

impl HandshakerSink {
    fn new(send: Sender<SocketAddr>) -> HandshakerSink {
        HandshakerSink{ send: send }
    }
}

impl Sink for HandshakerSink {
    type SinkItem = SocketAddr;
    type SinkError = SendError<SocketAddr>;

    fn start_send(&mut self, item: SocketAddr) -> StartSend<SocketAddr, SendError<SocketAddr>> {
        self.send.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<SocketAddr>> {
        self.send.poll_complete()
    }
}

//----------------------------------------------------------------------------------//

pub struct HandshakerStream<T> where T: Transport {
    recv: Receiver<SocketAddr>,
    listener: T::Listener
}

impl Stream 