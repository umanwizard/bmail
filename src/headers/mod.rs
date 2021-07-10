use enum_kinds::EnumKind;
use std::borrow::Cow;

pub mod address;
pub mod layout;

use crate::parse::is_wsp;
use crate::{ByteStr, ByteString};
use address::{Address, Mailbox};

#[derive(Debug, Clone, EnumKind)]
#[enum_kind(HeaderFieldKind)]
pub enum HeaderFieldInner<'a> {
    Unstructured(ByteString),
    // "Date:"
    OrigDate(chrono::DateTime<chrono::offset::FixedOffset>),
    From(Vec<Mailbox<'a>>),
    Sender(Mailbox<'a>),
    ReplyTo(Vec<Address<'a>>),
    To(Vec<Address<'a>>),
    Cc(Vec<Address<'a>>),
    Bcc(Vec<Address<'a>>),
}

#[derive(Clone, Debug)]
pub struct HeaderField<'a> {
    name: &'a ByteStr,
    raw_value: &'a [u8],
    inner: HeaderFieldInner<'a>,
    cached_unfolded_value: Option<Cow<'a, ByteStr>>,
}

impl<'a> HeaderField<'a> {
    pub fn new(name: &'a ByteStr, raw_value: &'a [u8], inner: HeaderFieldInner<'a>) -> Self {
        Self {
            name,
            raw_value,
            inner,
            cached_unfolded_value: None,
        }
    }
    pub fn name(&self) -> &ByteStr {
        self.name
    }
    pub fn raw_value(&self) -> &[u8] {
        self.raw_value
    }
    pub fn inner(&self) -> &HeaderFieldInner<'_> {
        &self.inner
    }
    pub fn unfolded_value_uncached(&self) -> Cow<'a, ByteStr> {
        // unfolding - remove any \r\n that is immediately
        // followed by WSP
        let rv = self.raw_value;
        let mut breaks = rv.windows(3).filter_map(|win| {
            if win[0] == b'\r' && win[1] == b'\n' && is_wsp(win[2]) {
                Some(unsafe { win.as_ptr().offset_from(rv.as_ptr()) } as usize)
            } else {
                None
            }
        });
        match breaks.next() {
            Some(r#break) => {
                let mut result = (&rv[0..r#break]).to_vec();
                let mut start = r#break + 2;
                while let Some(r#break) = breaks.next() {
                    result.extend_from_slice(&rv[start..r#break]);
                    start = r#break + 2;
                }
                result.extend_from_slice(&rv[start..]);
                Cow::Owned(ByteString(result))
            }
            None => Cow::Borrowed(ByteStr::from_slice(rv)),
        }
    }
    pub fn unfolded_value(&mut self) -> &ByteStr {
        if self.cached_unfolded_value.is_none() {
            self.cached_unfolded_value = Some(self.unfolded_value_uncached());
        }
        &*self.cached_unfolded_value.as_ref().unwrap()
    }
}
