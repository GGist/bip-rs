use inner::{self, InnerBencode};
use inner::reference::{BInnerRef, InnerRef};
use reference::{TypeRef, BTypeRef};

/// Trait for generically working with an immutable Bencode list.
pub trait ListRef<'a, 'b>: TypeRef<'a, 'b> {
    /// Access an inde within thist list to get a Bencode value.
    fn get(&self, index: usize) -> Option<BTypeRef<'a, 'b>>;

    /// Get the length of the list.
    fn len(&self) -> usize;
}

//----------------------------------------------------------------------------//

impl<'a, 'b> ListRef<'a, 'b> for BListRef<'a, 'b> {
    fn get(&self, mut index: usize) -> Option<BTypeRef<'a, 'b>> {
        let inner = self.inner();
        let tokens_buffer = inner.tokens_buffer();

        let mut token_index = self.token_index();
        while let Some(new_index) = inner::next_type_token(tokens_buffer, token_index) {
            if index == 0 {
                return Some(BTypeRef::new(BInnerRef::new(inner, token_index)))
            }

            token_index = new_index;
            index -= 1;
        }

        None
    }

    fn len(&self) -> usize {
        let tokens_buffer = self.inner().tokens_buffer();

        let mut token_index = self.token_index();
        let mut length = 0;
        while let Some(new_index) = inner::next_type_token(tokens_buffer, token_index) {
            length += 1;
            token_index = new_index;
        }

        length
    }
}

//----------------------------------------------------------------------------//

/// Immutable Bencode list object.
pub struct BListRef<'a, 'b: 'a> {
    inner: BInnerRef<'a, 'b>
}

impl<'a, 'b> BListRef<'a, 'b> {
    pub fn new(inner: BInnerRef<'a, 'b>) -> BListRef<'a, 'b> {
        BListRef{ inner: inner }
    }
}

impl<'a, 'b> InnerRef<'a, &'b [u8]> for BListRef<'a, 'b> {
    fn inner(&self) -> &'a InnerBencode<'static, &'b [u8]> {
        self.inner.inner()
    }

    fn token_index(&self) -> usize {
        self.inner.token_index()
    }
}