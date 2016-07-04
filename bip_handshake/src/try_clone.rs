use std::io::Result;

use rotor::mio::tcp::TcpStream;

pub trait TryClone: Sized {
    fn try_clone(&self) -> Result<Self>;
}

impl TryClone for TcpStream {
    fn try_clone(&self) -> Result<Self> {
        self.try_clone()
    }
}
