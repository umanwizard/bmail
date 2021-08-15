use std::borrow::Cow;
use std::collections::HashMap;

use crate::ByteStr;

use quoted_printable::ParseMode;

#[derive(Clone, Debug)]
pub struct ContentType<'a> {
    pub r#type: &'a ByteStr,
    pub subtype: &'a ByteStr,
    pub parameters: HashMap<String, String>, // TODO [perf] - could avoid copies for the (typical) lowercase-only case.
}

#[derive(Copy, Clone, Debug)]
pub enum ContentTransferEncoding {
    SevenBit,
    EightBit,
    Binary,
    Base64,
    QuotedPrintable,
}

#[derive(Debug)]
pub enum ContentDecodeError {
    Base64(base64::DecodeError),
    QuotedPrintable(quoted_printable::QuotedPrintableError),
}

impl ContentTransferEncoding {
    pub fn decode(&self, input: Vec<u8>) -> Result<Vec<u8>, ContentDecodeError> {
        use ContentTransferEncoding::*;
        match self {
            SevenBit | EightBit | Binary => Ok(input),
            Base64 => base64::decode(input).map_err(ContentDecodeError::Base64),
            QuotedPrintable => quoted_printable::decode(input, ParseMode::Robust)
                .map_err(ContentDecodeError::QuotedPrintable),
        }
    }
    pub fn is_trivial(&self) -> bool {
        use ContentTransferEncoding::*;
        match self {
            SevenBit | EightBit | Binary => true,
            Base64 | QuotedPrintable => false,
        }
    }
}
