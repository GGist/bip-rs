/// Spawn a block worker thread.
///
/// Returns a channel to send work to the block worker thread.
pub fn spawn_block_worker(sender: Sender<GeneralWorkerMessage>) -> Sender<BlockWorkerMessage> {
    let (send, recv) = chan::async();

    thread::spawn(move || {
        let mut context = GeneralWorkerContext::new();

        for msg in recv {
            match msg {
                BlockWorkerMessage::ReserveBlock(namespace, request, hash, piece_msg) => {

                }
            }
        }
    });

    send
}