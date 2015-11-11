use std::borrow::{Cow};
use std::error::{Error};
use std::fmt::{self, Display, Formatter};
use std::io::{self};

use message::error::{ErrorMessage};

pub type DhtResult<T> = Result<T, DhtError>;

/// A list specifying the types of DhtErrors that may occur.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum DhtErrorKind {
    /// All Nodes Have Gone Stale.
    StaleNodes,
    /// Failed To Bootstrap Table.
    BootstrapFailed,
    /// A Node Sent Us An Invalid Message.
    InvalidMessage,
    /// A Node Sent Us An Invalid Request.
    InvalidRequest(ErrorMessage<'static>),
    /// A Node Sent Us An Invalid Response.
    ///
    /// Includes Error Responses.
    InvalidResponse,
    /// A Node Sent Us An Unexpected Response.
    UnsolicitedResponse,
    /// Some Other Error Occurred.
    Other
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct DhtError {
    kind: DhtErrorKind,
    desc: &'static str,
    detail: Option<Cow<'static, str>>
}

impl DhtError {
    pub fn new(kind: DhtErrorKind, desc: &'static str) -> DhtError {
        DhtError{ kind: kind, desc: desc, detail: None }
    }
    
    pub fn with_detail<T>(kind: DhtErrorKind, desc: &'static str, detail: T)
        -> DhtError where T: Into<Cow<'static, str>> {
        DhtError{ kind: kind, desc: desc, detail: Some(detail.into()) }
    }
    
    pub fn kind(&self) -> &DhtErrorKind {
        &self.kind
    }
    
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_ref().map(|x| &**x)
    }
}

impl Display for DhtError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        try!(f.write_fmt(format_args!("Kind: {:?}", self.kind)));
        
        try!(f.write_fmt(format_args!(", Description: {}", self.desc)));
        
        if let Some(detail) = self.detail.as_ref() {
            try!(f.write_fmt(format_args!(", Detail: {}", detail)));
        }
        
        Ok(())
    }   
}

impl From<io::Error> for DhtError {
    fn from(error: io::Error) -> DhtError {
        DhtError::with_detail(DhtErrorKind::Other,
            "An io::Error Occurred, See detail",
            error.description().to_owned()
        )
    }
}
/*
impl From<BencodeError> for DhtError {
    fn from(error: BencodeError) -> DhtError {
        DhtError::with_detail(DhtErrorKind::Other,
            "A BencodeError Occurred, See detail",
            error.to_string()
        )
    }
}

impl From<BencodeConvertError> for DhtError {
    fn from(error: BencodeConvertError) -> DhtError {
        DhtError::with_detail(DhtErrorKind::Other,
            "A BencodeConvertError Occurred, See detail",
            error.to_string()
        )
    }
}*/

impl Error for DhtError {
    fn description(&self) -> &str { self.desc }
    
    fn cause(&self) -> Option<&Error> { None }
}