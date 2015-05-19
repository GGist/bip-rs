trait ToFileBuilder {
    
}

struct TorrentBuilder {
    torrent_root: HashMap<String, Bencode>;
}

impl TorrentBuilder {
	fn from_file(file: File, piece_length: u32) -> IoResult<TorrentBuilder> {
        
    }
	
	fn from_directory(directory: File, piece_length: u32) IoResult<TorrentBuilder> {
        
    }

    
    fn build(self) -> Metainfo {
        
    }
    
    fn set_announce(&mut self, announce: &str) {
        self.torrent_root[ANNOUNCE_KEY] = Bencode::Bytes(announce.to_owned().into_bytes());
    }
    
    fn set_comment(&mut self, comment: &str) {
		self.torrent_root[COMMENT_KEY] = Bencode::Bytes(comment.to_owned().into_bytes());
    }
    
    fn set_created_by(&mut self, created_by: &str) {
		self.torrent_root[CREATED_BY_KEY] = Bencode::Bytes(created_by.to_owned().into_bytes());
    }
    
    fn set_creation_date(&mut self, creation_date: i32) {
		self.torrent_root[CREATION_DATE_KEY] = Bencode::Bytes(creation_date.to_owned().into_bytes());
    }
    
    fn set_private_tracker(&mut self, is_private: bool) {
		let mut info_root = torrent_root[INFO_KEY];
		
		info_root[PRIVATE_KEY] = if is_private Bencode::Int(1) else Bencode::Int(0);
    }

    fn add_extension<T>(&mut self, extension: T) -> bool
        where T: Fn(&mut HashMap<String, Bencode>) -> bool {
        extension(self.torrent_root)
    }
}