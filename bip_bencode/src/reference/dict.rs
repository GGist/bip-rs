use inner::InnerBencode;
use inner::reference::{BInnerRef, InnerRef};
use reference::{BTypeRef, TypeRef};
use reference::list::{BListRef, ListRef};

/// Trait for generically working with an immutable Bencode dictionary.
pub trait DictRef<'a, 'b>: TypeRef<'a, 'b> {
    /// Lookup a Bencode value in the dictionary by it's key.
    fn lookup(&self, key: &[u8]) -> Option<BTypeRef<'a, 'b>>;

    /// Get the length of the dictionary.
    fn len(&self) -> usize;
}

//----------------------------------------------------------------------------//

impl<'a, 'b> DictRef<'a, 'b> for BDictRef<'a, 'b> {
    fn lookup(&self, key: &[u8]) -> Option<BTypeRef<'a, 'b>> {
        // Pretend that the dictionary is a list, just compare every other value
        // for key equality. We unwrap ceratin things here because we know this
        // is a dict, so we can make certain assumptions about the list values.
        let blist_ref = BListRef::new(BInnerRef::new(self.inner(), self.token_index()));

        let mut curr_index = 0;
        while let Some(type_ref) = blist_ref.get(curr_index) {
            let bytes = type_ref.bytes().unwrap();

            if bytes == key {
                return blist_ref.get(curr_index + 1)
            }

            curr_index += 2;

        }
        
        None
    }

    fn len(&self) -> usize {
        // Pretend that the dictionary is a list, divide size by 2
        let blist_ref = BListRef::new(BInnerRef::new(self.inner(), self.token_index()));

        blist_ref.len() / 2
    }
}

//----------------------------------------------------------------------------//

/// Immutable Bencode dictionary object.
pub struct BDictRef<'a, 'b: 'a> {
    inner: BInnerRef<'a, 'b>
}

impl<'a, 'b> BDictRef<'a, 'b> {
    pub fn new(inner: BInnerRef<'a, 'b>) -> BDictRef<'a, 'b> {
        BDictRef{ inner: inner }
    }
}

impl<'a, 'b> InnerRef<'a, &'b [u8]> for BDictRef<'a, 'b> {
    fn inner(&self) -> &'a InnerBencode<'static, &'b [u8]> {
        self.inner.inner()
    }

    fn token_index(&self) -> usize {
        self.inner.token_index()
    }
}