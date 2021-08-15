pub mod error;
pub mod headers;
pub mod parse;

pub struct SmtpEnvelope {
    pub from: Option<String>,
    pub to: Option<String>,
}

use std::ops::Deref;

#[derive(Clone)]
pub struct ByteString(pub Vec<u8>);

pub struct ByteStr(pub [u8]);

impl ByteStr {
    pub fn from_slice(slice: &[u8]) -> &Self {
        unsafe { std::mem::transmute(slice) }
    }
}

impl std::fmt::Debug for ByteStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

impl std::fmt::Debug for ByteString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.deref())
    }
}

impl Deref for ByteString {
    type Target = ByteStr;

    fn deref(&self) -> &Self::Target {
        ByteStr::from_slice(self.0.as_slice())
    }
}

impl std::borrow::Borrow<ByteStr> for ByteString {
    fn borrow(&self) -> &ByteStr {
        self.deref()
    }
}

impl ToOwned for ByteStr {
    type Owned = ByteString;

    fn to_owned(&self) -> ByteString {
        ByteString(self.0.to_vec())
    }
}

#[derive(Clone)]
pub enum Body<'a> {
    SimpleText(String),
    SimpleBinary(Vec<u8>),
    Multipart {
        preamble: &'a [u8],
        parts: Vec<Message<'a>>,
        epilogue: &'a [u8],
        // content_subtype: &'a [u8],
    },
}

pub use headers::HeaderField;
#[derive(Clone)]
pub struct Message<'a> {
    header: Vec<HeaderField<'a>>,
    content_type: Option<usize>, // index of Content-Type field in header
    body: Body<'a>,
    size: usize,
}

impl<'a> Message<'a> {
    pub(crate) fn new(
        header: Vec<HeaderField<'a>>,
        content_type: Option<usize>,
        body: Body<'a>,
        size: usize,
    ) -> Self {
        Self {
            header,
            content_type,
            body,
            size,
        }
    }

    pub fn header(&self) -> &[HeaderField<'a>] {
        &self.header
    }

    pub fn body(&self) -> &Body<'a> {
        &self.body
    }
    pub fn size(&self) -> usize {
        self.size
    }
}

impl<'a> std::fmt::Debug for Body<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Body::SimpleText(text) => {
                writeln!(f, "TEXT BODY")?;
                write!(f, "{}", text)?;
            }
            Body::SimpleBinary(_) => {
                writeln!(f, "BINARY BODY (omitted)")?;
            }
            Body::Multipart {
                preamble,
                parts,
                epilogue,
            } => {
                writeln!(f, "MULTIPART BODY WITH {} PARTS", parts.len())?;
                if !preamble.is_empty() {
                    writeln!(f, "PREAMBLE")?;
                    write!(f, "{}", String::from_utf8_lossy(preamble))?;
                }
                for (i, p) in parts.iter().enumerate() {
                    writeln!(f, "PART {}", i)?;
                    write!(f, "{:?}", p)?;
                }
                if !epilogue.is_empty() {
                    writeln!(f, "EPILOGUE")?;
                    write!(f, "{}", String::from_utf8_lossy(epilogue))?;
                }
            }
        }
        Ok(())
    }
}

impl<'a> std::fmt::Debug for Message<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        for hf in self.header.iter() {
            writeln!(f, "{:?}:{:?}", hf.name(), hf.inner())?;
        }
        write!(f, "\n{:?}", self.body)?;
        Ok(())
    }
}
