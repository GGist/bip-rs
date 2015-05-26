use bencode::{BencodeView};
use error::{TorrentResult, TorrentErrorKind, TorrentError};
use torrent::metainfo::{self};
use util::{Dictionary};

/// Torrent parser that parses values on each access.
pub struct LazyMetainfo;

/// Slice from a BencodeView object to another type.
///
/// Returns a WrongType error if it cannot convert to the given type.
macro_rules! slice_ben {
    ($ben:ident, $key:expr, $f:expr) => (
        match $f($ben) {
            Some(n) => n,
            None => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
        }
    )
}

/// Slice from a Dictionary object to a converted BencodeView object.
///
/// Returns a MissingKey error if the value is not in the dictionary or a
/// WrongType error if the value cannot be converted from the BencodeView object.
macro_rules! slice_dict {
    ($dict:ident, $key:expr, $f:expr) => (
        match $dict.lookup($key) {
            Some(n) => match $f(n) {
                Some(n) => n,
                None => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
            },
            None => return Err(TorrentError::new(TorrentErrorKind::MissingKey, $key))
        }
    );
}

/// Optionally slice from a Dictionary object to a converted BencodeView object.
///
/// Returns a WrongType error if the value cannot be converted from the BencodeView 
/// object.
macro_rules! slice_dict_opt {
    ($dict:ident, $key:expr, $f:expr) => (
        match $dict.lookup($key) {
            Some(n) => match $f(n) {
                Some(n) => Some(n),
                None => return Err(TorrentError::new(TorrentErrorKind::WrongType, $key))
            },
            None => None
        }
    );
}

/// Tries to convert the root BencodeView value to a Dictionary.
pub fn slice_root_dict<'a, T>(root: &'a T) -> TorrentResult<&'a Dictionary<String, T>>
    where T: BencodeView<InnerItem=T> {
    Ok(slice_ben!(root, metainfo::ROOT_IDENT, BencodeView::dict))
}
