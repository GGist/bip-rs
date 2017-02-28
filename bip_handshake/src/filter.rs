/// Trait for accepting `HandshakeFilter`s which drive the filtering behavior for peer handshakes.
pub trait HandshakeFilters {
    /// Add the filter to the current set of filters.
    fn add_filter<F>(&self, filter: F)
        where F: HandshakeFilter + Hash + PartialEq + Eq;

    /// Remove the filter from the current set of filters.
    fn remove_filter<F>(&self, filter: F)
        where F: HandshakeFilter + Hash + PartialEq + Eq;

    /// Clear all filters currently set.
    fn clear_filter(&self);
}

//----------------------------------------------------------------------------------//

/// Trait for deciding whether or not to allow a handshake to proceed based on fields
/// related to the handshake. Some fields are available before a peer connection is even
/// made, so as an optimization, we may pass in `Option::None` to see if the filter can
/// make a filter decision on the given field without the data for that field.
///
/// By default, all methods will return `FilterDecision::Pass` so that implementers filtering
/// on only a few fields only have to implement the methods for those fields.
///
/// In order for a handshake to pass the filter, each field has to be either not blocked, or 
/// effectively "whitelisted" (see `FilterDecision::Allow`).
pub trait HandshakeFilter {
    /// Make a filter decision based on the peer `SocketAddr`.
    fn by_addr(opt_addr: Option<SocketAddr>) -> FilterDecision { FilterDecision::Pass }

    /// Make a filter decision based on the handshake `Protocol`.
    fn by_prot(opt_prot: Option<Protocol>) -> FilterDecision { FilterDecision::Pass }

    /// Make a filter decision based on the `ExtensionBits`.
    fn by_ext(opt_ext: Option<ExtensionBits>) -> FilterDecision { FilterDecision::Pass }

    /// Make a filter decision based on the `InfoHash`.
    fn by_hash(opt_hash: Option<InfoHash>) -> FilterDecision { FilterDecision::Pass }

    /// Make a filter decision based on the `PeerId`.
    fn by_pid(opt_pid: Option<PeerId>) -> FilterDecision { FilterDecision::Pass }
}

//----------------------------------------------------------------------------------//

/// Decision made when deciding to filter a handshake based on some handshake data.
enum FilterDecision {
    /// Filter needs the given data to make a decision.
    NeedData,
    /// Pass on making a filter decision for the given field.
    Pass,
    /// Block the handshake based on the given data.
    Block,
    /// Allow the handshake based on the given data.
    ///
    /// Allowing a field that a previous filter blocked
    /// will have a whitelisting effect, where the block
    /// will be overriden.
    Allow
}