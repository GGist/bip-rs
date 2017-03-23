use bip_util::bt::PeerId;

/// Trait for advertisement information that other peers can discover.
pub trait DiscoveryInfo {
    /// Retrieve our public port that we advertise to others.
    fn port(&self) -> u16;

    /// Retrieve our `PeerId` that we advertise to others.
    fn peer_id(&self) -> PeerId;
}