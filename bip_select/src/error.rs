//! Module for uber error types.

use crate::discovery::error::{DiscoveryError, DiscoveryErrorKind};

error_chain! {
    types {
        UberError, UberErrorKind, UberResultExt;
    }

    links {
        Discovery(DiscoveryError, DiscoveryErrorKind);
    }
}
