use std::io;

use rotor::mio::tcp::TcpStream;

/// Trait for cloning an object which may not be cloneable.
pub trait TryClone: Sized {
    /// Attempt to clone the object.
    fn try_clone(&self) -> io::Result<Self>;
}

impl TryClone for TcpStream {
    fn try_clone(&self) -> io::Result<Self> {
        self.try_clone()
    }
}
