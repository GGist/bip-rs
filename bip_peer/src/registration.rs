use bip_util::send::TrySender;

pub trait LayerRegistration<S1: Send, S2: Send> {
    type SS2: TrySender<S2>;

    fn register(&self, send: Box<TrySender<S1>>) -> Self::SS2;
}
