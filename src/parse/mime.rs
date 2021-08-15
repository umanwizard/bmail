use std::borrow::Cow;
use std::collections::HashMap;

use super::cfws;
use super::is_vchar;
use super::quoted_string;
use crate::headers::mime::{ContentTransferEncoding, ContentType};
use crate::{ByteStr, ByteString};

use nom::error::VerboseError;

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::bytes::complete::take_while1;
use nom::combinator::map;
use nom::combinator::opt;
use nom::multi::fold_many0;
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

fn r#type(input: &[u8]) -> IResult<&[u8], &ByteStr, VerboseError<&[u8]>> {
    map(take_while1(is_content_type_ch), ByteStr::from_slice)(input)
}

fn subtype(input: &[u8]) -> IResult<&[u8], &ByteStr, VerboseError<&[u8]>> {
    map(take_while1(is_content_type_ch), ByteStr::from_slice)(input)
}

fn is_token_ch(ch: u8) -> bool {
    is_vchar(ch) && !b"()<>@,;:\\\"/[]?=".iter().any(|ch2| *ch2 == ch)
}

fn parameter(input: &[u8]) -> IResult<&[u8], (String, String), VerboseError<&[u8]>> {
    let attribute = take_while1(is_token_ch);
    let value = alt((
        map(
            take_while1(is_token_ch),
            |s: &[u8]| String::from_utf8(s.to_vec()).unwrap(), // perf - unsafe ?
        ),
        map(quoted_string, |s: ByteString| {
            String::from_utf8(s.0).unwrap()
        }), // perf - unsafe ?
    ));

    let (input, (attr, _, val)) =
        tuple((attribute, delimited(opt(cfws), tag(b"="), opt(cfws)), value))(input)?;
    Ok((input, (String::from_utf8(attr.to_vec()).unwrap(), val)))
}

pub(crate) fn content_transfer_encoding(
    input: &[u8],
) -> IResult<&[u8], ContentTransferEncoding, VerboseError<&[u8]>> {
    use ContentTransferEncoding::*;
    delimited(
        opt(cfws),
        alt((
            map(tag_no_case(b"7bit"), |_| SevenBit),
            map(tag_no_case(b"8bit"), |_| EightBit),
            map(tag_no_case(b"binary"), |_| Binary),
            map(tag_no_case(b"base64"), |_| Base64),
            map(tag_no_case(b"quoted-printable"), |_| QuotedPrintable),
        )),
        opt(cfws),
    )(input)
}

pub(crate) fn content_type(input: &[u8]) -> IResult<&[u8], ContentType<'_>, VerboseError<&[u8]>> {
    let (input, (r#type, _, subtype, parameters, _)) = tuple((
        preceded(opt(cfws), r#type),
        preceded(opt(cfws), tag(b"/")),
        preceded(opt(cfws), subtype),
        fold_many0(
            preceded(
                tuple((opt(cfws), tag(b";"))),
                preceded(opt(cfws), parameter),
            ),
            HashMap::new(),
            |mut params, (k, v)| {
                params.insert(k, v);
                params
            },
        ),
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
