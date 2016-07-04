struct HandshakeConnection {
    
}

enum HandshakeState {
    Initiate(InitiateState, Option<PeerId>),
    Complete(CompleteState)
}

enum InitiateState {
    WriteMessage,
    ReadLength,
    ReadMessage
}

enum CompleteState {
    ReadLength,
    ReadMessage,
    WriteMessage
}

impl Protocol for HandshakeConnection {
    type Context = ;
    type Socket = TcpStream;
    type Seed = (HandshakeState, Rc<RefCell<(InfoHash, PeerId)>>);

    fn create(seed: Self::Seed, sock: &mut Self::Socket, scope: &mut Scope<Self::Context>) -> Intent<Self> {

    }

    fn bytes_read(self, transport: &mut Transport<Self::Socket>, end: usize, scope: &mut Scope<Self::Context>) -> Intent<Self> {

    }

    fn bytes_flushed(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {

    }

    fn timeout(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {

    }

    fn exception(self, _transport: &mut Transport<Self::Socket>, reason: Exception, _scope: &mut Scope<Self::Context>) -> Intent<Self> {

    }

    fn fatal(self, reason: Exception, scope: &mut Scope<Self::Context>) -> Option<Box<Error>> {

    }

    fn wakeup(self, transport: &mut Transport<Self::Socket>, scope: &mut Scope<Self::Context>) -> Intent<Self> {

    }
}

/// Returns Some(true) if the remote handshake is valid, Some(false) if the remote handshake is invalid, or None if more bytes need to be read.
fn parse_remote_handshake(bytes: &[u8], expected_pid: Option<PeerId>, expected_protocol: &'static str) -> ParseStatus {
    let parse_result = chain!(bytes,
        _unused_prot: call!(parse_remote_protocol, expected_protocol) ~
        _unused_ext:  take!(RESERVED_BYTES_LEN) ~
        hash:         call!(parse_remote_hash) ~
        pid:          call!(parse_remote_pid, expected_pid) ,
        || { (hash, pid) }
    );

    match parse_result {
        IResult::Done(_, (hash, pid)) => ParseStatus::Valid(hash, pid),
        IResult::Error(_) => ParseStatus::Invalid,
        IResult::Incomplete(_) => ParseStatus::More,
    }
}

fn parse_remote_protocol<'a>(bytes: &'a [u8], expected_protocol: &'static str) -> IResult<&'a [u8], &'a [u8]> {
    let expected_length = expected_protocol.len() as u8;

    switch!(bytes, map!(be_u8, |len| len == expected_length),
        true => tag!(expected_protocol.as_bytes())
    )
}

fn parse_remote_hash(bytes: &[u8]) -> IResult<&[u8], InfoHash> {
    map!(bytes, take!(bt::INFO_HASH_LEN), |hash| InfoHash::from_hash(hash).unwrap())
}

fn parse_remote_pid(bytes: &[u8], opt_expected_pid: Option<PeerId>) -> IResult<&[u8], PeerId> {
    if let Some(expected_pid) = opt_expected_pid {
        map!(bytes, tag!(expected_pid.as_ref()), |id| PeerId::from_hash(id).unwrap())
    } else {
        map!(bytes, take!(bt::PEER_ID_LEN), |id| PeerId::from_hash(id).unwrap())
    }
}
