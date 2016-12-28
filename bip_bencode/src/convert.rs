use bencode::{Bencode};
use error::{BencodeConvertErrorKind, BencodeConvertError};
use dictionary::{Dictionary};

/// Trait for casting bencode objects and converting conversion errors into application specific errors.
pub trait BencodeConvert {
    type Error;

    /// Convert the given conversion error into the appropriate error type.
    fn handle_error(&self, error: BencodeConvertError) -> Self::Error;
    
    /// Attempt to convert the given bencode value into an integer.
    ///
    /// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_int<'a, E>(&self, bencode: &Bencode<'a>, error_key: E) -> Result<i64, Self::Error>
        where E: AsRef<[u8]> {
        bencode.int().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not An Integer", error_key.as_ref().to_owned())))
    }
    
    /// Attempt to convert the given bencode value into bytes.
    ///
    /// EError key is used to generate an appropriate error message should the operation return an error.
    fn convert_bytes<'a, E>(&self, bencode: &Bencode<'a>, error_key: E) -> Result<&'a [u8], Self::Error>
        where E: AsRef<[u8]> {
        bencode.bytes().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not Bytes", error_key.as_ref().to_owned())))
    }
    
    /// Attempt to convert the given bencode value into a UTF-8 string.
    ///
    /// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_str<'a, E>(&self, bencode: &Bencode<'a>, error_key: E) -> Result<&'a str, Self::Error>
        where E: AsRef<[u8]> {
        bencode.str().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not A String", error_key.as_ref().to_owned())))
    }
    
    /// Attempty to convert the given bencode value into a list.
    ///
    /// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_list<'a, 'b, E>(&self, bencode: &'b Bencode<'a>, error_key: E)
        -> Result<&'b [Bencode<'a>], Self::Error> where E: AsRef<[u8]> {
        bencode.list().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not A List", error_key.as_ref().to_owned())))
    }
    
    /// Attempt to convert the given bencode value into a dictionary.
    ///
    /// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_dict<'a, 'b, E>(&self, bencode: &'b Bencode<'a>, error_key: E)
        -> Result<&'b Dictionary<'a, Bencode<'a>>, Self::Error> where E: AsRef<[u8]> {
        bencode.dict().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not A Dictionary", error_key.as_ref().to_owned())))
    }
    
    /// Look up a value in a dictionary of bencoded values using the given key.
    fn lookup<'a, 'b, K>(&self, dictionary: &'b Dictionary<'a, Bencode<'a>>, key: K)
        -> Result<&'b Bencode<'a>, Self::Error> where K: AsRef<[u8]> {
        let key_ref = key.as_ref();

        match dictionary.lookup(key_ref) {
            Some(n) => Ok(n),
            None    => Err(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::MissingKey, 
                "Dictionary Missing Key", key_ref.to_owned())))
        }
    }
    
    /// Combines a lookup operation on the given key with a conversion of the value, if found, to an integer.
    fn lookup_and_convert_int<'a, K>(&self, dictionary: &Dictionary<'a, Bencode<'a>>, key: K)
        -> Result<i64, Self::Error> where K: AsRef<[u8]> {
        self.convert_int(try!(self.lookup(dictionary, &key)), &key)
    }
    
    /// Combines a lookup operation on the given key with a conversion of the value, if found, to a series of bytes.
    fn lookup_and_convert_bytes<'a, K>(&self, dictionary: &Dictionary<'a, Bencode<'a>>, key: K)
        -> Result<&'a [u8], Self::Error> where K: AsRef<[u8]> {
        self.convert_bytes(try!(self.lookup(dictionary, &key)), &key)
    }
    
    /// Combines a lookup operation on the given key with a conversion of the value, if found, to a UTF-8 string.
    fn lookup_and_convert_str<'a, K>(&self, dictionary: &Dictionary<'a, Bencode<'a>>, key: K)
        -> Result<&'a str, Self::Error> where K: AsRef<[u8]> {
        self.convert_str(try!(self.lookup(dictionary, &key)), &key)
    }
    
    /// Combines a lookup operation on the given key with a conversion of the value, if found, to a list.
    fn lookup_and_convert_list<'a: 'b, 'b, K>(&self, dictionary: &'b Dictionary<'a, Bencode<'a>>, key: K)
        -> Result<&'b [Bencode<'a>], Self::Error> where K: AsRef<[u8]> {
        self.convert_list(try!(self.lookup(dictionary, &key)), &key)
    }
    
    /// Combines a lookup operation on the given key with a conversion of the value, if found, to a dictionary.
    fn lookup_and_convert_dict<'a: 'b, 'b, K>(&self, dictionary: &'b Dictionary<'a, Bencode<'a>>, key: K)
        -> Result<&'b Dictionary<'a, Bencode<'a>>, Self::Error> where K: AsRef<[u8]> {
        self.convert_dict(try!(self.lookup(dictionary, &key)), &key)
    }
}
