use std::str;

use inner::{self, InnerBencode, BencodeToken, StartIndex, EndIndex, SliceIndices, MatchesStart};
use error::{BencodeParseResult, BencodeParseError, BencodeParseErrorKind};

/// Decodes the given list of bytes at the given position into a bencoded structures.
///
/// Panic only occurs is a programming error occurred.
pub fn decode<'a>(bytes: &'a [u8], mut pos: usize) -> BencodeParseResult<InnerBencode<'static, &'a [u8]>> {
    let mut tokens = Vec::new();

    while pos != bytes.len() {
        let (next_token, next_pos) = try!(decode_token(bytes, pos));

        tokens.push(next_token);

        pos = next_pos;
    }

    // Go back and update the start list/dict tokens with the correct end positions into the buffer
    try!(update_end_positions(&mut tokens));
    // We don't do check while building the structure, go back and do these
    try!(validate_tokens(&tokens));

    Ok(InnerBencode::new(bytes, tokens))
}

/// Runs validation on the given tokens to make sure that the bencode bytes produced by the
/// tokens list is going to be valid.
fn validate_tokens<'a>(tokens: &[BencodeToken<'a>]) -> BencodeParseResult<()> {
    // Note: A lot of these checks may be O(n^2) in complexity, but this whole token approach
    // in general is not exactly supposed to be fast on paper. Dictionarys and lists having
    // linear random access and such. The benefit is that our tokens buffer should always be
    // in cache, and also the fact that in the real world, this bencode won't be a huge structure.
    try!(validate_non_empty(&tokens));
    try!(validate_type_recursion(&tokens));
    try!(validate_keys_have_values(&tokens));
    try!(validate_keys_are_bytes(&tokens));
    //try!(validate_keys_sorted_and_unique(&tokens));

    Ok(())
}

/// Decodes the next bencode token, and return the token, as well as the next position to parse.
fn decode_token<'a>(bytes: &'a [u8], pos: usize) -> BencodeParseResult<(BencodeToken<'static>, usize)> {
    let curr_byte = try!(peek_byte(bytes, pos));

    match curr_byte {
        ::INT_START => {
            let (value, new_pos) = try!(decode_int(bytes, pos + 1, ::BEN_END));

            Ok((BencodeToken::Int(value, StartIndex(pos), EndIndex(new_pos - 1)), new_pos))
        }
        ::LIST_START => Ok((BencodeToken::ListStart(StartIndex(pos), EndIndex(0)), pos + 1)),
        ::DICT_START => Ok((BencodeToken::DictStart(StartIndex(pos), EndIndex(0)), pos + 1)),
        ::BYTE_LEN_LOW...::BYTE_LEN_HIGH => {
            let ((start, end), new_pos) = try!(decode_bytes(bytes, pos));
            
            // Include the length digit, don't increment position
            Ok((BencodeToken::BytesPos(SliceIndices(start, end), StartIndex(pos), EndIndex(0)), new_pos))
        },
        ::BEN_END => Ok((BencodeToken::End(StartIndex(pos), MatchesStart(false)), pos + 1)),
        _ => {
            Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidByte { pos: pos }))
        }
    }
}


/// Return the integer as well as the starting byte of the next type.
fn decode_int(bytes: &[u8], pos: usize, delim: u8) -> BencodeParseResult<(i64, usize)> {
    let (_, begin_decode) = bytes.split_at(pos);

    let relative_end_pos = match begin_decode.iter().position(|n| *n == delim) {
        Some(end_pos) => end_pos,
        None => {
            return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntNoDelimiter{ pos: pos }))
        }
    };
    let int_byte_slice = &begin_decode[..relative_end_pos];

    if int_byte_slice.len() > 1 {
        // Negative zero is not allowed (this would not be caught when converting)
        if int_byte_slice[0] == b'-' && int_byte_slice[1] == b'0' {
            return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntNegativeZero{ pos: pos }));
        }

        // Zero padding is illegal, and unspecified for key lengths (we disallow both)
        if int_byte_slice[0] == b'0' {
            return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntZeroPadding{ pos: pos }));
        }
    }

    let int_str = match str::from_utf8(int_byte_slice) {
            Ok(n)  => n,
            Err(_) => {
            return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntParseError{ pos: pos }))
        }
    };

    // Position of end of integer type, next byte is the start of the next value
    let absolute_end_pos = pos + relative_end_pos;
    match i64::from_str_radix(int_str, 10) {
        Ok(n)  => Ok((n, absolute_end_pos + 1)),
        Err(_) => {
            Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidIntParseError {
                pos: pos,
            }))
        }
    }
}

/// Returns the byte reference as well as the starting byte of the next type.
fn decode_bytes<'a>(bytes: &'a [u8], pos: usize) -> BencodeParseResult<((usize, usize), usize)> {
    let (num_bytes, start_pos) = try!(decode_int(bytes, pos, ::BYTE_LEN_END));

    if num_bytes < 0 {
        return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidLengthNegative {
            pos: pos,
        }));
    }

    // Should be safe to cast to usize (TODO: Check if cast would overflow to provide
    // a more helpful error message, otherwise, parsing will probably fail with an
    // unrelated message).
    let num_bytes = num_bytes as usize;

    if num_bytes > bytes[start_pos..].len() {
        return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidLengthOverflow {
            pos: pos,
        }));
    }

    let end_pos = start_pos + num_bytes;
    Ok(((start_pos, end_pos), end_pos))
}

/// Updates the end position into the buffer that start tokens are tracking. As a side effect,
/// also validates that each start (list/dict) token has a corresponding end token.
fn update_end_positions<'a>(mut tokens: &mut [BencodeToken<'a>]) -> BencodeParseResult<()> {
    fn find_end_bytes_position<'a>(tokens: &mut [BencodeToken<'a>], token_pos: usize, start_bytes_pos: usize) -> BencodeParseResult<usize> {
        let mut curr_index = token_pos + 1;

        while let Some(next_index) = inner::next_type_token(tokens, curr_index) {
            curr_index = next_index;
        }

        if let Some(&mut BencodeToken::End(StartIndex(end_pos), MatchesStart(ref mut has_match))) = tokens.get_mut(curr_index) {
            *has_match = true;

            Ok(end_pos)
        } else {
            Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidUnmatchedStart{ pos: start_bytes_pos }))
        }
    }
    let mut token_position = 0;

    while token_position < tokens.len() {
        let start_token = tokens[token_position];

        match start_token {
            BencodeToken::ListStart(StartIndex(start_pos), EndIndex(_)) => {
                let real_end_pos = try!(find_end_bytes_position(&mut tokens, token_position, start_pos));
                
                tokens[token_position] = BencodeToken::ListStart(StartIndex(start_pos), EndIndex(real_end_pos));
            }
            BencodeToken::DictStart(StartIndex(start_pos), EndIndex(_)) => {
                let real_end_pos = try!(find_end_bytes_position(&mut tokens, token_position, start_pos));

                tokens[token_position] = BencodeToken::DictStart(StartIndex(start_pos), EndIndex(real_end_pos));
            },
            BencodeToken::End(StartIndex(start_pos), MatchesStart(has_match)) if !has_match => {
                return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidUnmatchedEnd{ pos: start_pos }))
            },
            _ => ()
        }

        token_position += 1;
    }

    Ok(())
}

/// Validates that the list of tokens is not empty.
fn validate_non_empty<'a>(tokens: &[BencodeToken<'a>]) -> BencodeParseResult<()> {
    if tokens.is_empty() {
        Err(BencodeParseError::from_kind(BencodeParseErrorKind::BytesEmpty{ pos: 0 }))
    } else {
        Ok(())
    }
}

/// Validates that the bencode is well formed, ie, no types side by side in the same recursive level.
fn validate_type_recursion<'a>(tokens: &[BencodeToken<'a>]) -> BencodeParseResult<()> {
    let expected_last_token_index = match tokens[0] {
        BencodeToken::ListStart(_, _) |
        BencodeToken::DictStart(_, _) => {
            let mut curr_index = 1;

            while let Some(next_index) = inner::next_type_token(tokens, curr_index) {
                curr_index = next_index;
            }

            curr_index
        },
        BencodeToken::End(_, _)         |
        BencodeToken::Int(_, _, _)      |
        BencodeToken::BytesRef(_)       |
        BencodeToken::BytesPos(_, _, _) => 0
    };

    if expected_last_token_index + 1 != tokens.len() {
        let StartIndex(start_pos) = inner::start_index_from_token(&tokens[expected_last_token_index + 1]);

        Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidByte{ pos: start_pos }))
    } else {
        Ok(())
    }
}

/// Validates that every dictionary key has a corresponding value.
fn validate_keys_have_values<'a>(tokens: &[BencodeToken<'a>]) -> BencodeParseResult<()> {
    for (index, token) in tokens.iter().enumerate() {
        match token {
            &BencodeToken::DictStart(_, _) => {
                let mut curr_index = index + 1;
                let mut items_count = 0;

                while let Some(next_index) = inner::next_type_token(tokens, curr_index) {
                    items_count += 1;
                    curr_index = next_index;
                }

                if items_count % 2 != 0 {
                    let StartIndex(start_pos) = inner::start_index_from_token(&tokens[curr_index]);

                    return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidValueExpected{ pos: start_pos }))
                }
            },
            _ => ()
        }
    }

    Ok(())
}

/// Validates that all dictionary keys are of bytes types.
fn validate_keys_are_bytes<'a>(tokens: &[BencodeToken<'a>]) -> BencodeParseResult<()> {
    for (index, token) in tokens.iter().enumerate() {
        match token {
            &BencodeToken::DictStart(_, _) => {
                let mut curr_index = index + 1;
                let mut expect_key = true;

                while let Some(next_index) = inner::next_type_token(tokens, curr_index) {
                    if expect_key {
                        match tokens[curr_index] {
                            BencodeToken::BytesPos(_, _, _) => (),
                            _                               => {
                                let StartIndex(start_pos) = inner::start_index_from_token(&tokens[curr_index]);

                                return Err(BencodeParseError::from_kind(BencodeParseErrorKind::InvalidBytesExpected{ pos: start_pos }))
                            }
                        }
                    }
                    
                    curr_index = next_index;
                    expect_key = !expect_key;
                }
            },
            _ => ()
        }
    }

    Ok(())
}

/// Validates that all dictionary keys are sorted and unique.
///
/// Currently uniqueness is tied to the keys being sorted because it would be really inefficient
/// to check uniqueness of keys, aside from allocating a HashSet like data structure.
fn validate_keys_sorted_and_unique<'a>(_tokens: &[BencodeToken<'a>]) -> BencodeParseResult<()> {
    unimplemented!()
}

/// Peek the next byte in the byte slice given, otherwise, throw an error.
fn peek_byte(bytes: &[u8], pos: usize) -> BencodeParseResult<u8> {
    bytes.get(pos)
        .map(|n| *n)
        .ok_or(BencodeParseError::from_kind(BencodeParseErrorKind::BytesEmpty { pos: pos }))
}

#[cfg(test)]
mod tests {
    use reference::{BencodeRef, TypeRef};
    use reference::dict::DictRef;
    use reference::list::ListRef;

    // Positive Cases
    const GENERAL: &'static [u8] = b"d0:12:zero_len_key8:location17:udp://test.com:8011:nested dictd4:listli-500500eee6:numberi500500ee";
    const RECURSION: &'static [u8] = b"lllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllleeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const BYTES_UTF8: &'static [u8] = b"16:valid_utf8_bytes";
    const DICTIONARY: &'static [u8] = b"d9:test_dictd10:nested_key12:nested_value11:nested_listli500ei-500ei0eee8:test_key10:test_valuee";
    const LIST: &'static [u8] = b"l10:test_bytesi500ei0ei-500el12:nested_bytesed8:test_key10:test_valueee";
    const BYTES: &'static [u8] = b"5:\xC5\xE6\xBE\xE6\xF2";
    const BYTES_ZERO_LEN: &'static [u8] = b"0:";
    const INT: &'static [u8] = b"i500e";
    const INT_NEGATIVE: &'static [u8] = b"i-500e";
    const INT_ZERO: &'static [u8] = b"i0e";

    // Negative Cases
    const BYTES_NEG_LEN: &'static [u8] = b"-4:test";
    const BYTES_EXTRA: &'static [u8] = b"l15:processed_bytese17:unprocessed_bytes";
    const BYTES_NOT_UTF8: &'static [u8] = b"5:\xC5\xE6\xBE\xE6\xF2";
    const INT_NAN: &'static [u8] = b"i500a500e";
    const INT_LEADING_ZERO: &'static [u8] = b"i0500e";
    const INT_DOUBLE_ZERO: &'static [u8] = b"i00e";
    const INT_NEGATIVE_ZERO: &'static [u8] = b"i-0e";
    const INT_DOUBLE_NEGATIVE: &'static [u8] = b"i--5e";
    const INT_MULTIPLE: &'static [u8] = b"i0ei0e";
    const DICT_UNORDERED_KEYS: &'static [u8] = b"d5:z_key5:value5:a_key5:valuee";
    const DICT_DUP_KEYS_SAME_DATA: &'static [u8] = b"d5:a_keyi0e5:a_keyi0ee";
    const DICT_DUP_KEYS_DIFF_DATA: &'static [u8] = b"d5:a_keyi0e5:a_key7:a_valuee";

    #[test]
    fn positive_decode_general() {
        let bencode = BencodeRef::decode(GENERAL).unwrap();

        let ben_dict = bencode.type_ref().dict_ref().unwrap();
        assert_eq!(ben_dict.lookup("".as_bytes()).unwrap().str().unwrap(),
                   "zero_len_key");
        assert_eq!(ben_dict.lookup("location".as_bytes()).unwrap().str().unwrap(),
                   "udp://test.com:80");
        assert_eq!(ben_dict.lookup("number".as_bytes()).unwrap().int().unwrap(),
                   500500i64);

        let nested_dict = ben_dict.lookup("nested dict".as_bytes()).unwrap().dict_ref().unwrap();
        let nested_list = nested_dict.lookup("list".as_bytes()).unwrap().list_ref().unwrap();
        assert_eq!(nested_list.get(0).unwrap().int().unwrap(), -500500i64);
    }

    #[test]
    fn positive_decode_recursion() {
        let _ = BencodeRef::decode(RECURSION).unwrap();

        // As long as we didnt overflow our call stack, we are good!
    }

    #[test]
    fn positive_decode_bytes_utf8() {
        let bencode = BencodeRef::decode(BYTES_UTF8).unwrap();

        assert_eq!(bencode.type_ref().str().unwrap(), "valid_utf8_bytes");
    }

    #[test]
    fn positive_decode_dict() {
        let bencode = BencodeRef::decode(DICTIONARY).unwrap();
        let dict = bencode.type_ref().dict_ref().unwrap();
        assert_eq!(dict.lookup("test_key".as_bytes()).unwrap().str().unwrap(),
                   "test_value");

        let nested_dict = dict.lookup("test_dict".as_bytes()).unwrap().dict_ref().unwrap();
        assert_eq!(nested_dict.lookup("nested_key".as_bytes()).unwrap().str().unwrap(),
                   "nested_value");

        let nested_list = nested_dict.lookup("nested_list".as_bytes()).unwrap().list_ref().unwrap();
        assert_eq!(nested_list.get(0).unwrap().int().unwrap(), 500i64);
        assert_eq!(nested_list.get(1).unwrap().int().unwrap(), -500i64);
        assert_eq!(nested_list.get(2).unwrap().int().unwrap(), 0i64);
    }

    #[test]
    fn positive_decode_list() {
        let bencode = BencodeRef::decode(LIST).unwrap();
        let list = bencode.type_ref().list_ref().unwrap();

        assert_eq!(list.get(0).unwrap().str().unwrap(), "test_bytes");
        assert_eq!(list.get(1).unwrap().int().unwrap(), 500i64);
        assert_eq!(list.get(2).unwrap().int().unwrap(), 0i64);
        assert_eq!(list.get(3).unwrap().int().unwrap(), -500i64);

        let nested_list = list.get(4).unwrap().list_ref().unwrap();
        assert_eq!(nested_list.get(0).unwrap().str().unwrap(), "nested_bytes");

        let nested_dict = list.get(5).unwrap().dict_ref().unwrap();
        assert_eq!(nested_dict.lookup("test_key".as_bytes()).unwrap().str().unwrap(),
                   "test_value");
    }

    #[test]
    fn positive_decode_bytes() {
        let (start, end) = super::decode_bytes(BYTES, 0).unwrap().0;
        let bytes = &BYTES[start..end];
        assert_eq!(bytes.len(), 5);
        assert_eq!(bytes[0] as char, 'Å');
        assert_eq!(bytes[1] as char, 'æ');
        assert_eq!(bytes[2] as char, '¾');
        assert_eq!(bytes[3] as char, 'æ');
        assert_eq!(bytes[4] as char, 'ò');
    }

    #[test]
    fn positive_decode_bytes_zero_len() {
        let (start, end) = super::decode_bytes(BYTES_ZERO_LEN, 0).unwrap().0;
        let bytes = &BYTES_ZERO_LEN[start..end];
        assert_eq!(bytes.len(), 0);
    }

    #[test]
    fn positive_decode_int() {
        let int_value = super::decode_int(INT, 1, ::BEN_END).unwrap().0;
        assert_eq!(int_value, 500i64);
    }

    #[test]
    fn positive_decode_int_negative() {
        let int_value = super::decode_int(INT_NEGATIVE, 1, ::BEN_END).unwrap().0;
        assert_eq!(int_value, -500i64);
    }

    #[test]
    fn positive_decode_int_zero() {
        let int_value = super::decode_int(INT_ZERO, 1, ::BEN_END).unwrap().0;
        assert_eq!(int_value, 0i64);
    }

    #[test]
    #[should_panic]
    fn negative_decode_bytes_neg_len() {
        BencodeRef::decode(BYTES_NEG_LEN).unwrap();
    }

    #[test]
    #[should_panic]
    fn negative_decode_bytes_extra() {
        BencodeRef::decode(BYTES_EXTRA).unwrap();
    }

    #[test]
    #[should_panic]
    fn negative_decode_bytes_not_utf8() {
        let bencode = BencodeRef::decode(BYTES_NOT_UTF8).unwrap();

        bencode.type_ref().str().unwrap();
    }

    #[test]
    #[should_panic]
    fn negative_decode_int_nan() {
        super::decode_int(INT_NAN, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_int_leading_zero() {
        super::decode_int(INT_LEADING_ZERO, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_int_double_zero() {
        super::decode_int(INT_DOUBLE_ZERO, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_int_negative_zero() {
        super::decode_int(INT_NEGATIVE_ZERO, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_int_double_negative() {
        super::decode_int(INT_DOUBLE_NEGATIVE, 1, ::BEN_END).unwrap().0;
    }

    #[test]
    #[should_panic]
    fn negative_decode_dict_unordered_keys() {
        BencodeRef::decode(DICT_UNORDERED_KEYS).unwrap();
    }

    #[test]
    #[should_panic]
    fn negative_decode_dict_dup_keys_same_data() {
        BencodeRef::decode(DICT_DUP_KEYS_SAME_DATA).unwrap();
    }

    #[test]
    #[should_panic]
    fn negative_decode_dict_dup_keys_diff_data() {
        BencodeRef::decode(DICT_DUP_KEYS_DIFF_DATA).unwrap();
    }

    #[test]
    #[should_panic]
    fn negative_mutliple_ints() {
        BencodeRef::decode(INT_MULTIPLE).unwrap();
    }
}
