use bip_bencode::{BencodeRef, BDictAccess, BConvert, BencodeConvertError, BListAccess};

use error::{ParseError, ParseResult};

/// Struct implemented the BencodeConvert trait for decoding the metainfo file.
struct MetainfoConverter;

impl BConvert for MetainfoConverter {
    type Error = ParseError;

    fn handle_error(&self, error: BencodeConvertError) -> ParseError {
        error.into()
    }
}

/// Global instance for our conversion struct.
const CONVERT: MetainfoConverter = MetainfoConverter;

/// Used as an error key to refer to the root bencode object.
pub const ROOT_ERROR_KEY: &'static [u8] = b"root";

/// Keys found within the root dictionary of a metainfo file.
pub const ANNOUNCE_URL_KEY:  &'static [u8] = b"announce";
pub const CREATION_DATE_KEY: &'static [u8] = b"creation date";
pub const COMMENT_KEY:       &'static [u8] = b"comment";
pub const CREATED_BY_KEY:    &'static [u8] = b"created by";
pub const ENCODING_KEY:      &'static [u8] = b"encoding";
pub const INFO_KEY:          &'static [u8] = b"info";

/// Keys found within the info dictionary of a metainfo file.
pub const PIECE_LENGTH_KEY: &'static [u8] = b"piece length";
pub const PIECES_KEY:       &'static [u8] = b"pieces";
pub const PRIVATE_KEY:      &'static [u8] = b"private";
pub const NAME_KEY:         &'static [u8] = b"name";
pub const FILES_KEY:        &'static [u8] = b"files";

/// Keys found within the files dictionary of a metainfo file.
pub const LENGTH_KEY: &'static [u8] = b"length";
pub const MD5SUM_KEY: &'static [u8] = b"md5sum";
pub const PATH_KEY:   &'static [u8] = b"path";

/// Parses the root bencode as a dictionary.
pub fn parse_root_dict<'a, 'b>(root_bencode: &'b BencodeRef<'a>)
                               -> ParseResult<&'b BDictAccess<'a, BencodeRef<'a>>> {
    CONVERT.convert_dict(root_bencode, ROOT_ERROR_KEY)
}

/// Parses the announce url from the root dictionary.
pub fn parse_announce_url<'a>(root_dict: &BDictAccess<'a, BencodeRef<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, ANNOUNCE_URL_KEY).ok()
}

/// Parses the creation date from the root dictionary.
pub fn parse_creation_date<'a>(root_dict: &BDictAccess<'a, BencodeRef<'a>>) -> Option<i64> {
    CONVERT.lookup_and_convert_int(root_dict, CREATION_DATE_KEY).ok()
}

/// Parses the comment from the root dictionary.
pub fn parse_comment<'a>(root_dict: &BDictAccess<'a, BencodeRef<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, COMMENT_KEY).ok()
}

/// Parses the created by from the root dictionary.
pub fn parse_created_by<'a>(root_dict: &BDictAccess<'a, BencodeRef<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, CREATED_BY_KEY).ok()
}

/// Parses the encoding from the root dictionary.
pub fn parse_encoding<'a>(root_dict: &BDictAccess<'a, BencodeRef<'a>>) -> Option<&'a str> {
    CONVERT.lookup_and_convert_str(root_dict, ENCODING_KEY).ok()
}

/// Parses the info dictionary from the root dictionary.
pub fn parse_info_bencode<'a, 'b>(root_dict: &'b BDictAccess<'a, BencodeRef<'a>>)
                               -> ParseResult<&'b BencodeRef<'a>> {
    CONVERT.lookup(root_dict, INFO_KEY)
}

// ----------------------------------------------------------------------------//

/// Parses the piece length from the info dictionary.
pub fn parse_piece_length<'a>(info_dict: &BDictAccess<'a, BencodeRef<'a>>) -> ParseResult<u64> {
    CONVERT.lookup_and_convert_int(info_dict, PIECE_LENGTH_KEY).map(|len| len as u64)
}

/// Parses the pieces from the info dictionary.
pub fn parse_pieces<'a>(info_dict: &BDictAccess<'a, BencodeRef<'a>>) -> ParseResult<&'a [u8]> {
    CONVERT.lookup_and_convert_bytes(info_dict, PIECES_KEY)
}

/// Parses the private flag from the info dictionary.
pub fn parse_private<'a>(info_dict: &BDictAccess<'a, BencodeRef<'a>>) -> bool {
    CONVERT.lookup_and_convert_int(info_dict, PRIVATE_KEY).ok().map_or(false, |p| p == 1)
}

/// Parses the name from the info dictionary.
pub fn parse_name<'a>(info_dict: &BDictAccess<'a, BencodeRef<'a>>) -> ParseResult<&'a str> {
    CONVERT.lookup_and_convert_str(info_dict, NAME_KEY)
}

/// Parses the files list from the info dictionary.
pub fn parse_files_list<'a, 'b>(info_dict: &'b BDictAccess<'a, BencodeRef<'a>>)
                                -> ParseResult<&'b BListAccess<BencodeRef<'a>>> {
    CONVERT.lookup_and_convert_list(info_dict, FILES_KEY)
}

// ----------------------------------------------------------------------------//

/// Parses the file dictionary from the file bencode.
pub fn parse_file_dict<'a, 'b>(file_bencode: &'b BencodeRef<'a>)
                               -> ParseResult<&'b BDictAccess<'a, BencodeRef<'a>>> {
    CONVERT.convert_dict(file_bencode, FILES_KEY)
}

/// Parses the length from the info or file dictionary.
pub fn parse_length<'a>(info_or_file_dict: &BDictAccess<'a, BencodeRef<'a>>) -> ParseResult<u64> {
    CONVERT.lookup_and_convert_int(info_or_file_dict, LENGTH_KEY).map(|len| len as u64)
}

/// Parses the md5sum from the info or file dictionary.
pub fn parse_md5sum<'a>(info_or_file_dict: &BDictAccess<'a, BencodeRef<'a>>) -> Option<&'a [u8]> {
    CONVERT.lookup_and_convert_bytes(info_or_file_dict, MD5SUM_KEY).ok()
}

/// Parses the path list from the file dictionary.
pub fn parse_path_list<'a, 'b>(file_dict: &'b BDictAccess<'a, BencodeRef<'a>>)
                               -> ParseResult<&'b BListAccess<BencodeRef<'a>>> {
    CONVERT.lookup_and_convert_list(file_dict, PATH_KEY)
}

/// Parses the path string from the path bencode.
pub fn parse_path_str<'a>(path_bencode: &BencodeRef<'a>) -> ParseResult<&'a str> {
    CONVERT.convert_str(path_bencode, PATH_KEY)
}
