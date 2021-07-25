use std::borrow::Cow;

use crate::ByteStr;

#[derive(Clone, Debug)]
pub struct ContentType<'a> {
    pub r#type: &'a ByteStr,
    pub subtype: &'a ByteStr,
    pub parameters: Vec<(&'a ByteStr, Cow<'a, ByteStr>)>,
}
