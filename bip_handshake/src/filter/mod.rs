use std::cmp::{PartialEq, Eq};
use std::net::SocketAddr;
use std::any::Any;

use message::protocol::Protocol;
use message::extensions::{Extensions};

use bip_util::bt::{InfoHash, PeerId};

pub mod filters;

/// Trait for adding and removing `HandshakeFilter`s.
pub trait HandshakeFilters {
    /// Add the filter to the current set of filters.
    fn add_filter<F>(&self, filter: F)
        where F: HandshakeFilter + PartialEq + Eq + 'static;

    /// Remove the filter from the current set of filters.
    fn remove_filter<F>(&self, filter: F)
        where F: HandshakeFilter + PartialEq + Eq + 'static;

    /// Clear all filters currently set.
    fn clear_filters(&self);
}

impl<'a, T> HandshakeFilters for &'a T where T: HandshakeFilters {
    fn add_filter<F>(&self, filter: F)
        where F: HandshakeFilter + PartialEq + Eq + 'static {
        (*self).add_filter(filter)
    }

    fn remove_filter<F>(&self, filter: F)
        where F: HandshakeFilter + PartialEq + Eq + 'static {
        (*self).remove_filter(filter)
    }

    fn clear_filters(&self) {
        (*self).clear_filters()
    }
}

//----------------------------------------------------------------------------------//

/// Trait for filtering connections during handshaking.
///
/// By default, all methods will return `FilterDecision::Pass` so that implementers filtering
/// on only a few fields only have to implement the methods for those fields. Option is passed
/// because some filters may be able to block peers before a connection is made, if data is
/// required, return `FilterDecision::NeedData` when `None` is passed.
///
/// In order for a handshake to pass the filter, each field has to be either not blocked, or 
/// effectively "whitelisted" (see `FilterDecision::Allow`).
#[allow(unused)]
pub trait HandshakeFilter {
    /// Used to implement generic equality.
    ///
    /// Should typically just return `self`.
    fn as_any(&self) -> &Any;

    /// Make a filter decision based on the peer `SocketAddr`.
    fn on_addr(&self, opt_addr: Option<&SocketAddr>) -> FilterDecision { FilterDecision::Pass }

    /// Make a filter decision based on the handshake `Protocol`.
    fn on_prot(&self, opt_prot: Option<&Protocol>) -> FilterDecision { FilterDecision::Pass }

    /// Make a filter decision based on the `Extensions`.
    fn on_ext(&self, opt_ext: Option<&Extensions>) -> FilterDecision { FilterDecision::Pass }

    /// Make a filter decision based on the `InfoHash`.
    fn on_hash(&self, opt_hash: Option<&InfoHash>) -> FilterDecision { FilterDecision::Pass }

    /// Make a filter decision based on the `PeerId`.
    fn on_pid(&self, opt_pid: Option<&PeerId>) -> FilterDecision { FilterDecision::Pass }
}

//----------------------------------------------------------------------------------//

/// Filtering decision made for a given handshake.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FilterDecision {
    /// Pass on making a filter decision for the given field.
    Pass = 0,
    /// Block the handshake based on the given data.
    Block = 1,
    /// Filter needs the given data to make a decision.
    NeedData = 2,
    /// Allow the handshake based on the given data.
    ///
    /// Allowing a field that a previous filter blocked
    /// will have a whitelisting effect, where the block
    /// will be overriden.
    Allow = 3
}

impl FilterDecision {
    /// Choose between the current decision, and the other decision.
    ///
    /// Allow > NeedData > Block > Pass
    pub fn choose(&self, other: FilterDecision) -> FilterDecision {
        let self_num = *self as u8;
        let other_num = other as u8;

        if self_num > other_num {
            *self
        } else {
            other
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FilterDecision;

    #[test]
    fn positive_decision_choose_self() {
        let decision = FilterDecision::Block;

        assert_eq!(FilterDecision::Block, decision.choose(FilterDecision::Block));
    }

    #[test]
    fn positive_decision_choose_higher() {
        let decision = FilterDecision::Pass;

        assert_eq!(FilterDecision::NeedData, decision.choose(FilterDecision::NeedData));
    }

    #[test]
    fn positive_decision_keep_higher() {
        let decision = FilterDecision::NeedData;

        assert_eq!(FilterDecision::NeedData, decision.choose(FilterDecision::Pass));
    }
}