/// Trait for runtime type discovery of a Bencode type for mutable references.
pub trait TypeMut<'a, 'b> {
    /// Yields the backing buffer used for the current bencode type.
    ///
    /// A mutable reference is required because if any modifications were made to
    /// the bencode type, it MAY need to re-encode itself into the backing buffer
    /// before returning.
    fn buffer(&mut self) -> &'a [u8];

    /// Optionally yields a mutable bencode list.
    fn list_mut<'c>(&'c mut self) -> Option<BListMut<'a, 'c>>;

    /// Optionally yields an immutable bencode dictionary.
    fn dict_mut<'c>(&'c mut self) -> Option<BDictMut<'a, 'c>>;
}