error_chain! {
    types {
        BencodeParseError, BencodeParseErrorKind, BencodeParseResultExt, BencodeParseResult;
    }

    errors {
        BytesEmpty {
            pos: usize
         } {
            description("Incomplete Number Of Bytes")
            display("Incomplete Number Of Bytes At {:?}", pos)
        }
        InvalidByte {
            pos: usize
         } {
            description("Invalid Byte Found")
            display("Invalid Byte Found At {:?}", pos)
        }
        InvalidIntNoDelimiter {
            pos: usize
         } {
            description("Invalid Integer Found With No Delimiter")
            display("Invalid Integer Found With No Delimiter At {:?}", pos)
        }
        InvalidIntNegativeZero {
            pos: usize
         } {
            description("Invalid Integer Found As Negative Zero")
            display("Invalid Integer Found As Negative Zero At {:?}", pos)
        }
        InvalidIntZeroPadding {
            pos: usize
         } {
            description("Invalid Integer Found With Zero Padding")
            display("Invalid Integer Found With Zero Padding At {:?}", pos)
        }
        InvalidIntParseError {
            pos: usize
         } {
            description("Invalid Integer Found To Fail Parsing")
            display("Invalid Integer Found To Fail Parsing At {:?}", pos)
        }
        InvalidKeyOrdering {
            pos: usize,
            key: Vec<u8>
         } {
            description("Invalid Dictionary Key Ordering Found")
            display("Invalid Dictionary Key Ordering Found At {:?} For Key {:?}", pos, key)
        }
        InvalidKeyDuplicates {
            pos: usize,
            key: Vec<u8>
         } {
            description("Invalid Dictionary Duplicate Keys Found")
            display("Invalid Dictionary Key Found At {:?} For Key {:?}", pos, key)
        }
        InvalidLengthNegative {
            pos: usize
         } {
            description("Invalid Byte Length Found As Negative")
            display("Invalid Byte Length Found As Negative At {:?}", pos)
        }
        InvalidLengthOverflow {
            pos: usize
         } {
            description("Invalid Byte Length Found To Overflow Native Size")
            display("Invalid Byte Length Found To Overflow Native Size At {:?}", pos)
        }
        InvalidUnmatchedEnd {
            pos: usize
        } {
            description("Invalid Un-Matched End Byte Found")
            display("Invalid Un-Matched End Byte Found At {:?}", pos)
        }
        InvalidUnmatchedStart {
            pos: usize
        } {
            description("Invalid Un-Matched Start Byte Found For List/Dict")
            display("Invalid Un-Matched Start Byte Found For List/Dict At {:?}", pos)
        }
        InvalidValueExpected {
            pos: usize
        } {
            description("Invalid Dictionary Key With No Value Found")
            display("Invalid Dictionary Key With No Value Found, Value Expected At {:?}", pos)
        }
        InvalidBytesExpected {
            pos: usize
        } {
            description("Invalid Dictionary Key As Non Byte Value")
            display("Invalid Dictionary Key As Non Byte Value At {:?}", pos)
        }
    }
}

error_chain! {
    types {
        BencodeConvertError, BencodeConvertErrorKind, BencodeConvertResultExt, BencodeConvertResult;
    }

    errors {
        MissingKey {
            key: Vec<u8>
         } {
            description("Missing Key In Bencode")
            display("Missing Key In Bencode For {:?}", key)
        }
        WrongType {
            key: Vec<u8>,
            expected_type: String
         } {
            description("Wrong Type In Bencode")
            display("Wrong Type In Bencode For {:?} Expected Type {}", key, expected_type)
        }
    }
}
