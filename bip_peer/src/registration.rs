use bip_util::sender::Sender;

pub trait LayerRegistration<S1: Send, S2: Send> {
    type SS2: Sender<S2>;

    fn register(&self, send: Box<Sender<S1>>) -> Self::SS2;
}
