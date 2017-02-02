use bip_util::send::TrySender;

/// Trait for registering one layer of the application with another.
pub trait LayerRegistration<S1: Send, S2: Send> {
    /// Allows the layer initiating the registration to specify a concrete type.
    type SS2: TrySender<S2>;

    /// Register the current layer with some receiving layer.
    fn register(&mut self, send: Box<TrySender<S1>>) -> Self::SS2;
}
