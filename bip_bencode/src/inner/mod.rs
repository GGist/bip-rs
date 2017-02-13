use std::str;

use decode;
use error::{BencodeParseResult, BencodeParseError, BencodeParseErrorKind};

pub mod reference;

/// InnerBencode which unifies both frontend types, BencodeRef and BencodeMut
/// so they can share the same decoding and encoding logic in the back end.
///
/// The benefit here is that on the front end, consumers of BencodeRef only
/// have to see the lifetime of the backing buffer (with a static lifetime
/// for tokens, which they don't have to see. And consumers of BencodeMut only
/// have to see the lifetime of the backing tokens (with a static lifetime
/// for the buffer).
pub struct InnerBencode<'a, T> {
    // Can't use Cow here because the underlying reference would be tied
    // to the lifetime of InnerBencode, and not the lifetime of the actual
    // buffer (since Cow dynamically switches lifetimes and has to take 
    // the lowest common denominator lifetime...)
    bytes_buffer:   T,
    tokens_buffer:  Vec<BencodeToken<'a>>
}

// Indices into the backing buffer for encoded slices.
#[derive(Copy, Clone, Debug)]
pub struct StartIndex(pub usize);
#[derive(Copy, Clone, Debug)]
pub struct EndIndex(pub usize);

// Indices into the backing buffer for a byte slice type.
#[derive(Copy, Clone, Debug)]
pub struct SliceIndices(pub usize, pub usize);

// Used for parsing, to mark an end token as having a start match.
#[derive(Copy, Clone, Debug)]
pub struct MatchesStart(pub bool);

#[derive(Copy, Clone, Debug)]
pub enum BencodeToken<'a> {
    ListStart(StartIndex, EndIndex),
    DictStart(StartIndex, EndIndex),
    End(StartIndex, MatchesStart),
    Int(i64, StartIndex, EndIndex),
    BytesRef(&'a [u8]),
    BytesPos(SliceIndices, StartIndex, EndIndex)
}

// Used for BencodeRef
impl<'a> InnerBencode<'static, &'a [u8]> {
    pub fn new(bytes: &'a [u8], tokens: Vec<BencodeToken<'static>>) -> InnerBencode<'static, &'a [u8]> {
        InnerBencode{ bytes_buffer: bytes, tokens_buffer: tokens }
    }

    pub fn with_buffer(bytes: &'a [u8]) -> BencodeParseResult<InnerBencode<'static, &'a [u8]>> {
        decode::decode(bytes, 0)
    }

    pub fn bytes_buffer(&self) -> &'a [u8] {
        self.bytes_buffer
    }
}

// Used for BencodeMut
impl<'a> InnerBencode<'a, ()> {
    pub fn with_tokens(tokens: Vec<BencodeToken<'a>>) -> InnerBencode<'a, ()> {
        InnerBencode{ bytes_buffer: (), tokens_buffer: tokens }
    }
}

impl<'a, T> InnerBencode<'a, T>  {
    pub fn tokens_buffer(&self) -> &[BencodeToken<'a>] {
        &self.tokens_buffer
    }
} 

//----------------------------------------------------------------------------//

/// Finds the start index into the buffer for the given token.
pub fn start_index_from_token<'a>(token: &BencodeToken<'a>) -> StartIndex {
    match token {
        &BencodeToken::ListStart(start, _)   => start,
        &BencodeToken::DictStart(start, _)   => start,
        &BencodeToken::End(start, _)         => start,
        &BencodeToken::Int(_, start, _)      => start,
        &BencodeToken::BytesRef(_)           => panic!("bip_bencode: BytesRef Has No Start Index"),
        &BencodeToken::BytesPos(_, start, _) => start
    }
}

/// Finds the next type token after the current one, skipping over recursive structures.
///
/// If none is returned, that means there is no next type token, and the current one most
/// likely points to the end of the current recursive structure (list or dict).
pub fn next_type_token<'a>(token_buffer: &[BencodeToken<'a>], index: usize) -> Option<usize> {
    match token_buffer.get(index) {
        Some(&BencodeToken::Int(_, _, _))      |
        Some(&BencodeToken::BytesRef(_))       |
        Some(&BencodeToken::BytesPos(_, _, _)) => Some(index + 1),
        Some(&BencodeToken::End(_, _))         |
        None                                   => None,
        Some(&BencodeToken::ListStart(_, _))   |
        Some(&BencodeToken::DictStart(_, _))   => {
            // Need to find the end so we know where the next index is
            let mut openings_found = 1;
            let mut curr_index = index + 1;

            loop {
                match token_buffer[curr_index] {
                    BencodeToken::End(_, _)       => openings_found -= 1,
                    BencodeToken::DictStart(_, _) |
                    BencodeToken::ListStart(_, _) => openings_found += 1,
                    _ => ()
                }

                curr_index += 1;
                if openings_found == 0 {
                    break;
                }
            }

            // Index is one past the end token OF THE NESTED ENTITY (not the original entity)
            Some(curr_index)
        }
    }
}