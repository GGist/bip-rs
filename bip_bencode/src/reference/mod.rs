use std::str;

use inner::{BencodeToken, SliceIndices, InnerBencode};
use inner::reference::{BInnerRef, InnerRef};
use reference::list::BListRef;
use reference::dict::BDictRef;
use error::BencodeParseResult;

pub mod dict;
pub mod list;

/// Immutable Bencode type object.
pub struct BencodeRef<'a> {
    inner: InnerBencode<'static, &'a [u8]>
}

impl<'b> BencodeRef<'b> {
    /// Decodes the given buffer into a `BencodeRef` object.
    pub fn decode(bytes: &'b [u8]) -> BencodeParseResult<BencodeRef<'b>> {
        InnerBencode::with_buffer(bytes).map(|inner| BencodeRef{ inner: inner })
    }

    /// Yields a type reference to the current Bencode object.
    ///
    /// This method exists to unify nested Bencode types with the base Bencode type.
    /// In essense, the `TypeRef` trait cannot be implemented for `BencodeRef` due to
    /// where the lifetime annotation is placed (method vs trait), so this method
    /// is used to retrieve a `BTypeRef` to the first Bencode object.
    pub fn type_ref<'a>(&'a self) -> BTypeRef<'a, 'b> {
        BTypeRef::new(BInnerRef::new(&self.inner, 0))
    }
}

//----------------------------------------------------------------------------//

/// Trait for runtime type discovery of a Bencode type for immutable references.
pub trait TypeRef<'a, 'b> {
    /// Optionally yields a bencode integer.
    fn int(&self) -> Option<i64>;

    /// Optionally yields a byte slice.
    fn bytes(&self) -> Option<&'b [u8]>;

    /// Optionally yields a str slice.
    fn str(&self) -> Option<&'b str>;

    /// Optionally yields an immutable bencode list.
    fn list_ref(&self) -> Option<BListRef<'a, 'b>>;

    /// Optionally yields an immutable bencode dictionary.
    fn dict_ref(&self) -> Option<BDictRef<'a, 'b>>;
}

//----------------------------------------------------------------------------//

impl<'a, 'b: 'a, T> TypeRef<'a, 'b> for T
    where T: InnerRef<'a, &'b [u8]> {
    fn int(&self) -> Option<i64> {
        let token_index = self.token_index();
        let inner = self.inner();
        let tokens_buffer = inner.tokens_buffer();

        match tokens_buffer.get(token_index) {
            Some(&BencodeToken::Int(value, _, _)) => Some(value),
            _ => None
        }
    }

    fn bytes(&self) -> Option<&'b [u8]> {
        let token_index = self.token_index();
        let inner = self.inner();
        let tokens_buffer = inner.tokens_buffer();
        
        // We could also match on BytesRef, but we know that a bencode reference
        // type is always serialized before being usable by ref traits, so we would
        // have switched out any BytesRef with BytesPos beforehand. Our unit
        // tests should be able to test this.
        match tokens_buffer.get(token_index) {
            Some(&BencodeToken::BytesPos(SliceIndices(start, end), _, _)) => {
                Some(&inner.bytes_buffer()[start..end])
            },
            _ => None
        }
    }

    fn str(&self) -> Option<&'b str> {
        self.bytes().and_then(|bytes| str::from_utf8(bytes).ok())
    }

    fn list_ref(&self) -> Option<BListRef<'a, 'b>> {
        let token_index = self.token_index();
        let inner = self.inner();
        let tokens_buffer = inner.tokens_buffer();
        
        match tokens_buffer.get(token_index) {
            Some(&BencodeToken::ListStart(_, _)) => {
                Some(BListRef::new(BInnerRef::new(inner, token_index + 1)))
            },
            _ => None
        }
    }

    fn dict_ref(&self) -> Option<BDictRef<'a, 'b>> {
        let token_index = self.token_index();
        let inner = self.inner();
        let tokens_buffer = inner.tokens_buffer();
        
        match tokens_buffer.get(token_index) {
            Some(&BencodeToken::DictStart(_, _)) => {
                Some(BDictRef::new(BInnerRef::new(inner, token_index + 1)))
            },
            _ => None
        }
    }
}

//----------------------------------------------------------------------------//

/// Immutable Bencode type reference object.
///
/// This object is used simillarly to `BencodeRef` but this object holds
/// a reference to the internal tokens instead of owning it, as `BencodeRef` does.
pub struct BTypeRef<'a, 'b: 'a> {
    inner: BInnerRef<'a, 'b>
}

impl<'a, 'b> BTypeRef<'a, 'b> {
    pub fn new(inner: BInnerRef<'a, 'b>) -> BTypeRef<'a, 'b> {
        BTypeRef{ inner: inner }
    }
}

impl<'a, 'b> InnerRef<'a, &'b [u8]> for BTypeRef<'a, 'b> {
    fn inner(&self) -> &'a InnerBencode<'static, &'b [u8]> {
        self.inner.inner()
    }

    fn token_index(&self) -> usize {
        self.inner.token_index()
    }
}