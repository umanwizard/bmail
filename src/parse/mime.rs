use std::borrow::Cow;

use super::cfws;
use super::is_vchar;
use super::quoted_string;
use crate::headers::mime::ContentType;
use crate::ByteStr;

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_while1;
use nom::combinator::map;
use nom::combinator::opt;
use nom::multi::many0;
use nom::sequence::delimited;
use nom::sequence::preceded;
use nom::sequence::tuple;
use nom::IResult;

// [RFC]: I just made this up. Is it specified anywhere?
fn is_content_type_ch(ch: u8) -> bool {
    (b'a' <= ch && ch <= b'z')
        || (b'A' <= ch && ch <= b'Z')
        || (b'0' <= ch && ch <= b'9')
        || ch == b'-'
        || ch == b'.'
        || ch == b'_'
}

fn r#type(input: &[u8]) -> IResult<&[u8], &ByteStr> {
    map(take_while1(is_content_type_ch), ByteStr::from_slice)(input)
}

fn subtype(input: &[u8]) -> IResult<&[u8], &ByteStr> {
    map(take_while1(is_content_type_ch), ByteStr::from_slice)(input)
}

fn is_token_ch(ch: u8) -> bool {
    is_vchar(ch) && !b"()<>@,;:\\\"/[]?=".iter().any(|ch2| *ch2 == ch)
}

fn parameter(input: &[u8]) -> IResult<&[u8], (&ByteStr, Cow<'_, ByteStr>)> {
    let attribute = take_while1(is_token_ch);
    let value = alt((
        map(take_while1(is_token_ch), |s| {
            Cow::Borrowed(ByteStr::from_slice(s))
        }),
        map(quoted_string, Cow::Owned),
    ));

    let (input, (attr, _, val)) =
        tuple((attribute, delimited(opt(cfws), tag(b"="), opt(cfws)), value))(input)?;
    Ok((input, (ByteStr::from_slice(attr), val)))
}

pub(crate) fn content_type(input: &[u8]) -> IResult<&[u8], ContentType<'_>> {
    let (input, (r#type, _, subtype, parameters, _)) = tuple((
        preceded(opt(cfws), r#type),
        preceded(opt(cfws), tag(b"/")),
        preceded(opt(cfws), subtype),
        many0(preceded(
            tuple((opt(cfws), tag(b";"))),
            preceded(opt(cfws), parameter),
        )),
        tuple((
            opt(tag(b";")), // [RFC] seen in the wild: trailing semicolon
            opt(cfws),
        )),
    ))(input)?;

    Ok((
        input,
        ContentType {
            r#type,
            subtype,
            parameters,
        },
    ))
}
