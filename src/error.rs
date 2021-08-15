use crate::headers::mime::ContentDecodeError;

type NomVerboseError<'a> = nom::error::VerboseError<&'a [u8]>;

#[derive(Debug)]
pub enum EmailError<'a> {
    Parse(NomVerboseError<'a>, Option<Box<EmailError<'a>>>),
    BadDate {
        y: u16,
        m: chrono::Month,
        d: u8,
    },
    BadTZOffset {
        is_east: bool,
        hh: u8,
        mm: u8,
    },
    BadDateTime {
        date: chrono::NaiveDate,
        tz: chrono::offset::FixedOffset,
        h: u8,
        m: u8,
        s: Option<u8>,
    },
    BadWeekday {
        date_time: chrono::DateTime<chrono::offset::FixedOffset>,
        weekday: chrono::Weekday,
    },
    ContentTypeWithoutBoundary,
    MultipartWithNontrivialCte,
    BodyDecode(ContentDecodeError),
    LineTooLong,
}

impl<'a> From<NomVerboseError<'a>> for EmailError<'a> {
    fn from(e: NomVerboseError<'a>) -> Self {
        Self::Parse(e, None)
    }
}

impl From<ContentDecodeError> for EmailError<'static> {
    fn from(e: ContentDecodeError) -> Self {
        Self::BodyDecode(e)
    }
}

impl<'a> nom::error::ParseError<&'a [u8]> for EmailError<'a> {
    fn from_error_kind(input: &'a [u8], code: nom::error::ErrorKind) -> Self {
        Self::Parse(NomVerboseError::from_error_kind(input, code), None)
    }
    fn append(input: &'a [u8], code: nom::error::ErrorKind, other: Self) -> Self {
        Self::Parse(
            NomVerboseError::from_error_kind(input, code),
            Some(Box::new(other)),
        )
    }
}
