use bip_bencode::{Bencode, Dictionary, BencodeConvert, BencodeConvertError, BencodeConvertErrorKind};
use bip_util::bt::{InfoHash};

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
pub const ROOT_ERROR_KEY: &'static str = "root";

/// Keys found within the root dictionary of a metainfo file.
pub const ANNOUNCE_URL_KEY:  &'static str = "announce";
pub const CREATION_DATE_KEY: &'static str = "creation date";
pub const COMMENT_KEY:       &'static str = "comment";
pub const CREATED_BY_KEY:    &'static str = "created by";
pub const ENCODING_KEY:      &'static str = "encoding";
pub const INFO_KEY:          &'static str = "info";

/// Keys found within the info dictionary of a metainfo file.
pub const PIECE_LENGTH_KEY: &'static str = "piece length";
pub const PIECES_KEY:       &'static str = "pieces";
pub const PRIVATE_KEY:      &'static str = "private";
pub const NAME_KEY:         &'static str = "name";
pub const FILES_KEY:        &'static str = "files";

/// Keys found within the files dictionary of a metainfo file.
pub const LENGTH_KEY: &'static str = "length";
pub const MD5SUM_KEY: &'static str = "md5sum";
pub const PATH_KEY:   &'static str = "path";

/// Parses the root bencode as a dictionary.
pub fn parse_root_dict<'a, 'b>(root_bencode: &'b Bencode<'a>) -> ParseResult<&'b Dictionary<'a, Bencode<'a>>> {
    CONVERT.convert_dict(root_bencode, ROOT_ERROR_KEY)
}

/// Parses the announce url from the root dictionary.
pub fn parse_announce_url<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, ANNOUNCE_URL_KEY).ok()
}

/// Parses the creation date from the root dictionary.
pub fn parse_creation_date<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> Option<i64> {
    CONVERT.lookup_and_convert_int(root_dict, CREATION_DATE_KEY).ok()
}

/// Parses the comment from the root dictionary.
pub fn parse_comment<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, COMMENT_KEY).ok()
}

/// Parses the created by from the root dictionary.
pub fn parse_created_by<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, CREATED_BY_KEY).ok()
}

/// Parses the encoding from the root dictionary.
pub fn parse_encoding<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, ENCODING_KEY).ok()
}

/// Parses the info dictionary from the root dictionary.
pub fn parse_info_dict<'a, 'b>(root_dict: &'b Dictionary<'a, Bencode<'a>>) -> ParseResult<&'b Dictionary<'a, Bencode<'a>>> {
    CONVERT.lookup_and_convert_dict(root_dict, INFO_KEY)
}

/// Parses the info hash from the root dictionary.
pub fn parse_info_hash<'a>(root_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<InfoHash> {
    let info_dict_bencode = try!(CONVERT.lookup(root_dict, INFO_KEY));
    let encoded_info_dict = info_dict_bencode.encode();
    
    Ok(InfoHash::from_bytes(&encoded_info_dict))
}

//----------------------------------------------------------------------------//

/// Parses the piece length from the info dictionary.
pub fn parse_piece_length<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<i64> {
    CONVERT.lookup_and_convert_int(info_dict, PIECE_LENGTH_KEY)
}

/// Parses the pieces from the info dictionary.
pub fn parse_pieces<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<&'a [u8]> {
    CONVERT.lookup_and_convert_bytes(info_dict, PIECES_KEY)
}

/// Parses the private flag from the info dictionary.
pub fn parse_private<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> bool {
    CONVERT.lookup_and_convert_int(info_dict, PRIVATE_KEY).ok().map_or(false, |p| p == 1)
}

/// Parses the name from the info dictionary.
pub fn parse_name<'a>(info_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<&'a str> {
    CONVERT.lookup_and_convert_str(info_dict, NAME_KEY)
}

/// Parses the files list from the info dictionary.
pub fn parse_files_list<'a, 'b>(info_dict: &'b Dictionary<'a, Bencode<'a>>) -> ParseResult<&'b [Bencode<'a>]> {
    CONVERT.lookup_and_convert_list(info_dict, FILES_KEY)
}

//----------------------------------------------------------------------------//

/// Parses the file dictionary from the file bencode.
pub fn parse_file_dict<'a, 'b>(file_bencode: &'b Bencode<'a>) -> ParseResult<&'b Dictionary<'a, Bencode<'a>>> {
    CONVERT.convert_dict(file_bencode, FILES_KEY)
}

/// Parses the length from the info or file dictionary.
pub fn parse_length<'a>(info_or_file_dict: &Dictionary<'a, Bencode<'a>>) -> ParseResult<i64> {
    CONVERT.lookup_and_convert_int(info_or_file_dict, LENGTH_KEY)
}

/// Parses the md5sum from the info or file dictionary.
pub fn parse_md5sum<'a>(info_or_file_dict: &Dictionary<'a, Bencode<'a>>) -> Option<&'a [u8]> {
    CONVERT.lookup_and_convert_bytes(info_or_file_dict, MD5SUM_KEY).ok()
}

/// Parses the path list from the file dictionary.
pub fn parse_path_list<'a, 'b>(file_dict: &'b Dictionary<'a, Bencode<'a>>) -> ParseResult<&'b [Bencode<'a>]> {
    CONVERT.lookup_and_convert_list(file_dict, PATH_KEY)
}

/// Parses the path string from the path bencode.
pub fn parse_path_str<'a>(path_bencode: &Bencode<'a>) -> ParseResult<&'a str> {
    CONVERT.convert_str(path_bencode, PATH_KEY)
}