use inner::InnerBencode;

pub trait InnerRef<'a, T> {
    fn inner(&self) -> &'a InnerBencode<'static, T>;
    fn token_index(&self) -> usize;
}

//----------------------------------------------------------------------------//

pub struct BInnerRef<'a, 'b: 'a> {
    inner: &'a InnerBencode<'static, &'b [u8]>,
    index: usize
}

impl<'a, 'b> BInnerRef<'a, 'b> {
    pub fn new(inner: &'a InnerBencode<'static, &'b [u8]>, index: usize) -> BInnerRef<'a, 'b> {
        BInnerRef{ inner: inner, index: index }
    }
}

impl<'a, 'b> InnerRef<'a, &'b [u8]> for BInnerRef<'a, 'b> {
    fn inner(&self) -> &'a InnerBencode<'static, &'b [u8]> {
        self.inner
    }

    fn token_index(&self) -> usize {
        self.index
    }
}