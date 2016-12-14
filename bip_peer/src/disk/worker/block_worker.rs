use chan;

/// Spawn a synchronous block worker thread.
///
/// Returns a channel to send work to the block worker thread.
pub fn spawn_sync_block_worker(clients: Arc<Clients>, blocks: Arc<Blocks>) -> Sender<SyncBlockMessage> {
    let (send, recv) = chan::async();

    thread::spawn(move || {
        for msg in recv {
            match msg {
                SyncBlockMessage::ReserveBlock(namespace, request, hash, piece_msg) => {
                    block.allocate_block(namespace, request, );
                }
            }
        }
    });

    send
}

/// Spawn an asynchronous block worker thread.
///
/// Returns a channel to send work to the block worker thread.
pub fn spawn_async_block_worker(blocks: Arc<Blocks>) -> Sender<AsyncBlockMessage> {
    let (send, recv) = chan::async();

    thread::spawn(move || {
        let mut context = GeneralWorkerContext::new();

        for msg in recv {
            handle_block_message();
            match msg {

                BlockWorkerMessage::ReserveBlock(namespace, request, hash, piece_msg) => {

                },
                BlockWorkerMessage::ReclaimBlock(namespace, request) => {

                }
            }
        }
    });

    send
}

fn handle_block_message(request: BlockMessage, clients: &Clients, blocks: &Blocks) {
    
}

    ReserveBlock(Token, Token, InfoHash, PieceMessage),
    ReclaimBlock(Token, Token)