struct HandshakeConnection {
    
}

enum HandshakeState {

}



impl Protocol for HandshakeConnection {
    type Context = ;
    type Socket = TcpStream;
    type Seed = TCPPeer;

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