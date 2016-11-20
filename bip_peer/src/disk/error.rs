quick_error! {
    #[derive(Clone, Debug, Hash, PartialEq, Eq)]
    pub enum RequestError {
        BlockTooBig {}
    } 
}

quick_error! {
    #[derive(Clone, Debug, Hash, PartialEq, Eq)]
    pub enum TorrentError {
        
    } 
}