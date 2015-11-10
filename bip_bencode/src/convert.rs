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
    fn convert_int<'a>(&self, bencode: &Bencode<'a>, error_key: &str) -> Result<i64, Self::Error> {
        bencode.int().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not An Integer", error_key.to_owned())))
    }
    
	/// Attempt to convert the given bencode value into bytes.
	///
	/// EError key is used to generate an appropriate error message should the operation return an error.
    fn convert_bytes<'a>(&self, bencode: &Bencode<'a>, error_key: &str) -> Result<&'a [u8], Self::Error> {
        bencode.bytes().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not Bytes", error_key.to_owned())))
    }
    
	/// Attempt to convert the given bencode value into a UTF-8 string.
	///
	/// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_str<'a>(&self, bencode: &Bencode<'a>, error_key: &str) -> Result<&'a str, Self::Error> {
        bencode.str().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not A String", error_key.to_owned())))
    }
    
	/// Attempty to convert the given bencode value into a list.
	///
	/// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_list<'a, 'b>(&self, bencode: &'b Bencode<'a>, error_key: &str)
        -> Result<&'b [Bencode<'a>], Self::Error> {
        bencode.list().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not A List", error_key.to_owned())))
    }
    
	/// Attempt to convert the given bencode value into a dictionary.
	///
	/// Error key is used to generate an appropriate error message should the operation return an error.
    fn convert_dict<'a, 'b>(&self, bencode: &'b Bencode<'a>, error_key: &str)
        -> Result<&'b Dictionary<'a, Bencode<'a>>, Self::Error> {
        bencode.dict().ok_or(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::WrongType,
            "Bencode Is Not A Dictionary", error_key.to_owned())))
    }
    
	/// Look up a value in a dictionary of bencoded values using the given key.
    fn lookup<'a, 'b>(&self, dictionary: &'b Dictionary<'a, Bencode<'a>>, key: &str)
        -> Result<&'b Bencode<'a>, Self::Error> {
        match dictionary.lookup(key) {
            Some(n) => Ok(n),
            None    => Err(self.handle_error(BencodeConvertError::with_key(BencodeConvertErrorKind::MissingKey, 
                "Dictionary Missing Key", key.to_owned())))
        }
    }
    
	/// Combines a lookup operation on the given key with a conversion of the value, if found, to an integer.
    fn lookup_and_convert_int<'a>(&self, dictionary: &Dictionary<'a, Bencode<'a>>, key: &str)
        -> Result<i64, Self::Error> {
        self.convert_int(try!(self.lookup(dictionary, key)), key)
    }
    
	/// Combines a lookup operation on the given key with a conversion of the value, if found, to a series of bytes.
    fn lookup_and_convert_bytes<'a>(&self, dictionary: &Dictionary<'a, Bencode<'a>>, key: &str)
        -> Result<&'a [u8], Self::Error> {
        self.convert_bytes(try!(self.lookup(dictionary, key)), key)
    }
    
	/// Combines a lookup operation on the given key with a conversion of the value, if found, to a UTF-8 string.
    fn lookup_and_convert_str<'a>(&self, dictionary: &Dictionary<'a, Bencode<'a>>, key: &str)
        -> Result<&'a str, Self::Error> {
        self.convert_str(try!(self.lookup(dictionary, key)), key)
    }
    
	/// Combines a lookup operation on the given key with a conversion of the value, if found, to a list.
    fn lookup_and_convert_list<'a: 'b, 'b>(&self, dictionary: &'b Dictionary<'a, Bencode<'a>>, key: &str)
        -> Result<&'b [Bencode<'a>], Self::Error> {
        self.convert_list(try!(self.lookup(dictionary, key)), key)
    }
    
	/// Combines a lookup operation on the given key with a conversion of the value, if found, to a dictionary.
    fn lookup_and_convert_dict<'a: 'b, 'b>(&self, dictionary: &'b Dictionary<'a, Bencode<'a>>, key: &str)
        -> Result<&'b Dictionary<'a, Bencode<'a>>, Self::Error> {
        self.convert_dict(try!(self.lookup(dictionary, key)), key)
    }
}
