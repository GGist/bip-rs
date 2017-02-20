use access::bencode::BRefAccess;
use error::{BencodeConvertErrorKind, BencodeConvertError};
use access::dict::BDictAccess;
use access::list::BListAccess;

/// Trait for casting bencode objects and converting conversion errors into application specific errors.
pub trait BConvert {
    type Error;

    /// Convert the given conversion error into the appropriate error type.
    fn handle_error(&self, error: BencodeConvertError) -> Self::Error;

    /// Attempt to convert the given bencode value into an integer.
    ///
    /// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_int<'a, B, E>(&self, bencode: B, error_key: E) -> Result<i64, Self::Error>
        where B: BRefAccess<'a>, E: AsRef<[u8]>
    {
        bencode.int().ok_or(self.handle_error(BencodeConvertError::from_kind(BencodeConvertErrorKind::WrongType{
            key: error_key.as_ref().to_owned(), expected_type: "Integer".to_owned()
        })))
    }

    /// Attempt to convert the given bencode value into bytes.
    ///
    /// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_bytes<'a, B, E>(&self, bencode: B, error_key: E) -> Result<&'a [u8], Self::Error>
        where B: BRefAccess<'a>, E: AsRef<[u8]>
    {
        bencode.bytes().ok_or(self.handle_error(BencodeConvertError::from_kind(BencodeConvertErrorKind::WrongType{
            key: error_key.as_ref().to_owned(), expected_type: "Bytes".to_owned()
        })))
    }

    /// Attempt to convert the given bencode value into a UTF-8 string.
    ///
    /// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_str<'a, B, E>(&self, bencode: B, error_key: E) -> Result<&'a str, Self::Error>
        where B: BRefAccess<'a>, E: AsRef<[u8]>
    {
        bencode.str().ok_or(self.handle_error(BencodeConvertError::from_kind(BencodeConvertErrorKind::WrongType{
            key: error_key.as_ref().to_owned(), expected_type: "UTF-8 Bytes".to_owned()
        })))
    }

    /// Attempty to convert the given bencode value into a list.
    ///
    /// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_list<'a: 'b, 'b, B, E>(&self, bencode: &'b B, error_key: E) -> Result<&'b BListAccess<B::BType>, Self::Error>
        where B: BRefAccess<'a>, E: AsRef<[u8]>
    {
        bencode.list().ok_or(self.handle_error(BencodeConvertError::from_kind(BencodeConvertErrorKind::WrongType{
            key: error_key.as_ref().to_owned(), expected_type: "List".to_owned()
        })))
    }

    /// Attempt to convert the given bencode value into a dictionary.
    ///
    /// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_dict<'a, 'b, B, E>(&self, bencode: &'b B, error_key: E) -> Result<&'b BDictAccess<'a, B::BType>, Self::Error>
        where B: BRefAccess<'a>, E: AsRef<[u8]>
    {
        bencode.dict().ok_or(self.handle_error(BencodeConvertError::from_kind(BencodeConvertErrorKind::WrongType{
            key: error_key.as_ref().to_owned(), expected_type: "Dictionary".to_owned()
        })))
    }

    /// Look up a value in a dictionary of bencoded values using the given key.
    fn lookup<'a, 'b, B, K>(&self, dictionary: &'b BDictAccess<'a, B>, key: K) -> Result<&'b B, Self::Error>
        where B: BRefAccess<'a>, K: AsRef<[u8]>
    {
        let key_ref = key.as_ref();

        match dictionary.lookup(key_ref) {
            Some(n) => Ok(n),
            None    => Err(self.handle_error(BencodeConvertError::from_kind(BencodeConvertErrorKind::MissingKey{ key: key_ref.to_owned() }))),
        }
    }

    /// Combines a lookup operation on the given key with a conversion of the value, if found, to an integer.
    fn lookup_and_convert_int<'a, B, K>(&self, dictionary: &BDictAccess<'a, B>, key: K) -> Result<i64, Self::Error>
        where B: BRefAccess<'a>, K: AsRef<[u8]>
    {
        self.convert_int(try!(self.lookup(dictionary, &key)), &key)
    }

    /// Combines a lookup operation on the given key with a conversion of the value, if found, to a series of bytes.
    fn lookup_and_convert_bytes<'a, B, K>(&self, dictionary: &BDictAccess<'a, B>, key: K) -> Result<&'a [u8], Self::Error>
        where B: BRefAccess<'a>, K: AsRef<[u8]>
    {
        self.convert_bytes(try!(self.lookup(dictionary, &key)), &key)
    }

    /// Combines a lookup operation on the given key with a conversion of the value, if found, to a UTF-8 string.
    fn lookup_and_convert_str<'a, B, K>(&self, dictionary: &BDictAccess<'a, B>, key: K) -> Result<&'a str, Self::Error>
        where B: BRefAccess<'a>, K: AsRef<[u8]>
    {
        self.convert_str(try!(self.lookup(dictionary, &key)), &key)
    }

    /// Combines a lookup operation on the given key with a conversion of the value, if found, to a list.
    fn lookup_and_convert_list<'a, 'b, B, K>(&self, dictionary: &'b BDictAccess<'a, B>, key: K) -> Result<&'b BListAccess<B::BType>, Self::Error>
        where B: BRefAccess<'a>, K: AsRef<[u8]>
    {
        self.convert_list(try!(self.lookup(dictionary, &key)), &key)
    }

    /// Combines a lookup operation on the given key with a conversion of the value, if found, to a dictionary.
    fn lookup_and_convert_dict<'a, 'b, B, K>(&self, dictionary: &'b BDictAccess<'a, B>, key: K) -> Result<&'b BDictAccess<'a, B::BType>, Self::Error>
        where B: BRefAccess<'a>, K: AsRef<[u8]>
    {
        self.convert_dict(try!(self.lookup(dictionary, &key)), &key)
    }
}
