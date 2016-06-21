struct TorrentClient {
    
}

impl TorrentClient {
    fn add(info: MetainfoFile) -> bool;
    fn add_with_opts(info: MetainfoFile, opts: TorrentOpts) -> bool;
    
    fn update_opts(info_hash: InfoHash, opts: TorrentOpts) -> bool;
    
    fn remove(info_hash: InfoHash) -> Option<MetainfoFile>;
}

struct TorrentOpts {
    strategy:       PieceStrategy,
    download_limit: u64,
    upload_limit:   u64
}

enum PieceStrategy {
    Leecher,
    Seeder,
    Streamer,
    SuperSeeder
}