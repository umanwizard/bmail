use nom::character::complete::crlf;
use nom::combinator::consumed;

use nom::combinator::recognize;
use nom::multi::fold_many0;
use nom::multi::fold_many_m_n;
use nom::multi::separated_list0;
use nom::sequence::terminated;
use nom::IResult;
use nom::Parser;

use super::header::header_field;
use super::satisfy_byte;

use crate::error::EmailError;
use crate::headers::mime::ContentType;
use crate::headers::HeaderFieldInner;
use crate::ByteStr;
use crate::Message;

fn is_text(ch: u8) -> bool {
    ch < 128 && ch != b'\r' && ch != b'\n'
}

fn text998(input: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(fold_many_m_n(
        0,
        998,
        satisfy_byte(is_text),
        (),
        |(), _ch| (),
    ))(input)
}

fn simple_body<'a>() -> impl Parser<&'a [u8], Vec<&'a [u8]>, nom::error::Error<&'a [u8]>> {
    separated_list0(crlf, text998).map(|mut v| {
        if v.last() == Some(&&b""[..]) {
            v.pop();
        }
        v
    })
}

fn multipart_body<'a, 'b>(
    boundary: &'b ByteStr,
) -> impl Parser<&'a [u8], Vec<&'a [u8]>, nom::error::Error<&'a [u8]>> + 'b
where
    'a: 'b,
{
    separated_list0(crlf, text998).map(move |mut v| {
        &boundary;
        if v.last() == Some(&&b""[..]) {
            v.pop();
        }
        v
    })
}

pub fn body<'a, 'b>(
    boundary: Option<&'b ByteStr>,
) -> impl Parser<&'a [u8], Vec<&'a [u8]>, EmailError<'a>> + 'b
where
    'a: 'b,
{
    move |input: &'a [u8]| -> IResult<&'a [u8], Vec<&'a [u8]>, EmailError<'a>> {
        match boundary {
            Some(boundary) => nom::Parser::into(multipart_body(boundary)).parse(input),
            _ => nom::Parser::into(simple_body()).parse(input),
        }
    }
}

pub fn message<'a>(input: &'a [u8]) -> IResult<&'a [u8], Message<'a>, EmailError<'a>> {
    use crate::HeaderField;
    let (i, (hfs, ctype_idx, _i)): (_, (Vec<HeaderField<'a>>, _, _)) = terminated(
        fold_many0(
            header_field,
            (vec![], None, 0),
            |(mut hfs, ctype_idx, i), hf| {
                let ctype_idx = ctype_idx.or_else(|| {
                    if let HeaderFieldInner::ContentType(_) = &hf.inner() {
                        Some(i)
                    } else {
                        None
                    }
                });
                hfs.push(hf);
                (hfs, ctype_idx, i + 1)
            },
        ),
        crlf,
    )(input)?;
    let boundary = match ctype_idx {
        Some(ctype_idx) => match hfs[ctype_idx].inner() {
            HeaderFieldInner::ContentType(ContentType {
                r#type, parameters, ..
            }) => {
                if r#type.0.eq_ignore_ascii_case(b"multipart") {
                    Some(
                        match parameters
                            .iter()
                            .filter_map(|(k, v)| {
                                if k.0.eq_ignore_ascii_case(b"boundary") {
                                    Some(&**v)
                                } else {
                                    None
                                }
                            })
                            .next()
                        {
                            Some(boundary) => boundary,
                            None => {
                                return Err(nom::Err::Error(EmailError::ContentTypeWithoutBoundary))
                            }
                        },
                    )
                } else {
                    None
                }
            }
            _ => unreachable!(),
        },
        None => None,
    };

    let (i, (body, body_lines)) = nom::Parser::into(consumed(body(boundary))).parse(i)?;
    Ok((
        i,
        Message::new(hfs, ctype_idx, body, body_lines, input.len()),
    ))
}
