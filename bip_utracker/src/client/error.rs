use error::ErrorResponse;

/// Result type for a ClientRequest.
pub type ClientResult<T> = Result<T, ClientError>;

/// Errors occuring as the result of a ClientRequest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// Request timeout reached.
    MaxTimeout,
    /// Request length exceeded the packet length.
    MaxLength,
    /// Client shut down the request client.
    ClientShutdown,
    /// Server sent us an invalid message.
    ServerError,
    /// Requested to send from IPv4 to IPv6 or vice versa.
    IPVersionMismatch,
    /// Server returned an error message.
    ServerMessage(ErrorResponse<'static>),
}
