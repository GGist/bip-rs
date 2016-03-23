//! Messaging primitives for server errors.

use std::borrow::{Cow};
use std::io::{self, Write};

use nom::{IResult};

///Error reported by the server and sent to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorResponse<'a> {
    message: Cow<'a, str>
}

impl<'a> ErrorResponse<'a> {
    /// Create a new ErrorResponse.
    pub fn new(message: &'a str) -> ErrorResponse<'a> {
        ErrorResponse{ message: Cow::Borrowed(message) }
    }
    
    /// Construct an ErrorResponse from the given bytes.
    pub fn from_bytes(bytes: &'a [u8]) -> IResult<&'a [u8], ErrorResponse<'a>> {
        map!(bytes, take_str!(bytes.len()), |m| ErrorResponse::new(m))
    }
    
    /// Write the ErrorResponse to the given writer.
    pub fn write_bytes<W>(&self, mut writer: W) -> io::Result<()>
        where W: Write {
        try!(writer.write_all(self.message.as_bytes()));
        
        Ok(())
    }
    
    /// Message describing the error that occured.
    pub fn message(&self) -> &str {
        &*self.message
    }
    
    /// Create an owned version of the ErrorResponse.
    pub fn to_owned(&self) -> ErrorResponse<'static> {
        ErrorResponse{ message: Cow::Owned((*self.message).to_owned()) }
    }
}