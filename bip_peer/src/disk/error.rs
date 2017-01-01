error_chain! {
    types {
        RequestError, RequestErrorKind, RequestResultExt, RequestResult;
    }
}

error_chain! {
    types {
        TorrentError, TorrentErrorKind, TorrentResultExt, TorrentResult;
    }
}