use bip_bencode::{Bencode, Dictionary, BencodeConvert, BencodeConvertError, BencodeConvertErrorKind};
use bip_util::bt::{InfoHash};
use url::{Url};

use error::{ParseError, ParseErrorKind, ParseResult};

/// Struct implemented the BencodeConvert trait for decoding the metainfo file.
struct MetainfoConverter;

impl BencodeConvert for MetainfoConverter {
    type Error = ParseError;
    
    fn handle_error(&self, error: BencodeConvertError) -> ParseError {
        let detail_msg = match error.kind() {
            BencodeConvertErrorKind::MissingKey => format!("Required Data Missing: {}", error.key()),
            BencodeConvertErrorKind::WrongType  => format!("Invalid Type For Data: {}", error.key())
        };
        
        ParseError::new(ParseErrorKind::MissingData, detail_msg)
    }
}

/// Global instance for our conversion struct.
const CONVERT: MetainfoConverter = MetainfoConverter;

/// Used as an error key to refer to the root bencode object.
const ROOT_ERROR_KEY: &'static str = "root";

/// Keys found within the root dictionary of a metainfo file.
const ANNOUNCE_URL_KEY:  &'static str = "announce";
const CREATION_DATE_KEY: &'static str = "creation date";
const COMMENT_KEY:       &'static str = "comment";
const CREATED_BY_KEY:    &'static str = "created by";
const ENCODING_KEY:      &'static str = "encoding";
const INFO_KEY:          &'static str = "info";

/// Keys found within the info dictionary of a metainfo file.
const PIECE_LENGTH_KEY: &'static str = "piece length";
const PIECES_KEY:       &'static str = "pieces";
const PRIVATE_KEY:      &'static str = "private";
const NAME_KEY:         &'static str = "name";
const FILES_KEY:        &'static str = "files";

/// Keys found within the files dictionary of a metainfo file.
const LENGTH_KEY: &'static str = "length";
const MD5SUM_KEY: &'static str = "md5sum";
const PATH_KEY:   &'static str = "path";

/// Decodes the root bencode as a dictionary.
pub fn decode_root_dict<'a, 'b>(root_bencode: &'b Bencode<'a>) -> ParseResult<&'b Dictionary<'a, Bencode<'a>>> {
    CONVERT.convert_dict(root_bencode, ROOT_ERROR_KEY)
}

/// Decodes the announce url from the root dictionary.
pub fn decode_announce_url<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<Url> {
    let tracker_url = try!(CONVERT.lookup_and_convert_str(root_dict, ANNOUNCE_URL_KEY));
    
    Url::parse(tracker_url).map_err(|_| {
        let err_msg = format!("Invalid Tracker Format: {}", tracker_url);
        
        ParseError::new(ParseErrorKind::InvalidData, err_msg)
    })
}

/// Decodes the creation date from the root dictionary.
pub fn decode_creation_date<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> Option<i64> {
    CONVERT.lookup_and_convert_int(root_dict, CREATION_DATE_KEY).ok()
}

/// Decodes the comment from the root dictionary.
pub fn decode_comment<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, COMMENT_KEY).ok()
}

/// Decodes the created by from the root dictionary.
pub fn decode_created_by<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, CREATED_BY_KEY).ok()
}

/// Decodes the encoding from the root dictionary.
pub fn decode_encoding<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, ENCODING_KEY).ok()
}

/// Decodes the info dictionary from the root dictionary.
pub fn decode_info_dict<'a, 'b>(root_dict: &'b Dictionary<'a, Bencode<'a>>) -> ParseResult<&'b Dictionary<'a, Bencode<'a>>> {
    CONVERT.lookup_and_convert_dict(root_dict, INFO_KEY)
}

/// Decodes the info hash from the root dictionary.
pub fn decode_info_hash<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<InfoHash> {
    let info_dict_bencode = try!(CONVERT.lookup(root_dict, INFO_KEY));
    let encoded_info_dict = info_dict_bencode.encode();
    
    Ok(InfoHash::from_bytes(&encoded_info_dict))
}

//----------------------------------------------------------------------------//

/// Decodes the piece length from the info dictionary.
pub fn decode_piece_length<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<i64> {
    CONVERT.lookup_and_convert_int(info_dict, PIECE_LENGTH_KEY)
}

/// Decodes the pieces from the info dictionary.
pub fn decode_pieces<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<&'a [u8]> {
    CONVERT.lookup_and_convert_bytes(info_dict, PIECES_KEY)
}

/// Decodes the private flag from the info dictionary.
pub fn decode_private<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> bool {
    CONVERT.lookup_and_convert_int(info_dict, PRIVATE_KEY).ok().map_or(false, |p| p == 1)
}

/// Decodes the name from the info dictionary.
pub fn decode_name<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<&'a str> {
    CONVERT.lookup_and_convert_str(info_dict, NAME_KEY)
}

/// Decodes the files list from the info dictionary.
pub fn decode_files_list<'a, 'b>(info_dict: &'b Dictionary<'a, Bencode<'a>>) -> ParseResult<&'b [Bencode<'a>]> {
    CONVERT.lookup_and_convert_list(info_dict, FILES_KEY)
}

//----------------------------------------------------------------------------//

/// Decodes the file dictionary from the file bencode.
pub fn decode_file_dict<'a, 'b>(file_bencode: &'b Bencode<'a>) -> ParseResult<&'b Dictionary<'a, Bencode<'a>>> {
    CONVERT.convert_dict(file_bencode, FILES_KEY)
}

/// Decodes the length from the info or file dictionary.
pub fn decode_length<'a>(info_or_file_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<i64> {
    CONVERT.lookup_and_convert_int(info_or_file_dict, LENGTH_KEY)
}

/// Decodes the md5sum from the info or file dictionary.
pub fn decode_md5sum<'a>(info_or_file_dict: &Dictionary<'a, Bencode<'a>>) -> Option<&'a [u8]> {
    CONVERT.lookup_and_convert_bytes(info_or_file_dict, MD5SUM_KEY).ok()
}

/// Decodes the path list from the file dictionary.
pub fn decode_path_list<'a, 'b>(file_dict: &'b Dictionary<'a, Bencode<'a>>) -> ParseResult<&'b [Bencode<'a>]> {
    CONVERT.lookup_and_convert_list(file_dict, PATH_KEY)
}

/// Decodes the path string from the path bencode.
pub fn decode_path_str<'a>(path_bencode: &Bencode<'a>) -> ParseResult<&'a str> {
    CONVERT.convert_str(path_bencode, PATH_KEY)
}