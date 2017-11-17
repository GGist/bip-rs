//! Module for uber error types.

use discovery::error::{DiscoveryError, DiscoveryErrorKind};

error_chain! {
    types {
        UberError, UberErrorKind, UberResultExt;
    }

    links {
        Discovery(DiscoveryError, DiscoveryErrorKind);
    }
}
