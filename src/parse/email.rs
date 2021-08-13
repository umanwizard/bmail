use nom::character::complete::crlf;
use nom::combinator::consumed;

use nom::combinator::recognize;
use nom::multi::fold_many0;
use nom::multi::fold_many_m_n;
use nom::multi::separated_list0;
use nom::sequence::terminated;
use nom::IResult;
use nom::Parser;
use regex::bytes::RegexBuilder;

use super::header::header_field;
use super::satisfy_byte;

use crate::error::EmailError;
use crate::headers::mime::ContentType;
use crate::headers::HeaderFieldInner;
use crate::Body;
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

#[derive(Debug, Eq, PartialEq)]
enum DelimiterType {
    BetweenParts,   // e.g. --DELIM
    EndOfContainer, // e.g. --DELIM--
}

fn match_delimiter(delim: &ByteStr, candidate: &[u8]) -> Option<DelimiterType> {
    let delim = &delim.0;
    if candidate.len() < 3 || &candidate[0..2] != &b"--"[..] {
        None
    } else if candidate.len() == delim.len() + 2 && &candidate[2..] == delim {
        Some(DelimiterType::BetweenParts)
    } else if candidate.len() == delim.len() + 4
        && &candidate[2..delim.len() + 2] == delim
        && &candidate[delim.len() + 2..] == &b"--"[..]
    {
        Some(DelimiterType::EndOfContainer)
    } else {
        None
    }
}

struct SimpleBodyResult<'a> {
    pub data: &'a [u8],
    pub lines: Vec<&'a [u8]>,
    pub ended_with_delim: Option<DelimiterType>,
}

fn simple_body<'a, 'b>(
    outer_boundary: Option<&'b ByteStr>,
) -> impl Parser<&'a [u8], SimpleBodyResult<'a>, nom::error::Error<&'a [u8]>> + 'b
where
    'a: 'b,
{
    move |mut i: &'a [u8]| {
        let mut lines = vec![];
        let orig_i = i;
        loop {
            if i.is_empty() {
                if outer_boundary.is_some() {
                    todo!()
                }
                return Ok((
                    i,
                    SimpleBodyResult {
                        data: orig_i,
                        lines,
                        ended_with_delim: None,
                    },
                ));
            }
            let (i2, line) = text998(i)?;
            let (i2, _) = crlf(i2)?;
            i = i2;
            match outer_boundary.and_then(|bound| match_delimiter(bound, line)) {
                Some(delim) => {
                    let last_line_has_crlf = lines.last() == Some(&&b""[..]);
                    if last_line_has_crlf {
                        lines.pop();
                    }
                    let body_len =
                        orig_i.len() - i.len() - (if last_line_has_crlf { 0 } else { 2 });
                    return Ok((
                        i,
                        SimpleBodyResult {
                            data: &orig_i[0..body_len],
                            lines,
                            ended_with_delim: Some(delim),
                        },
                    ));
                }
                None => lines.push(line),
            }
        }
    }
}

struct MultipartBodyResult<'a> {
    preamble: &'a [u8],
    parts: Vec<Message<'a>>,
    epilogue: &'a [u8],
    ended_with_delim: Option<DelimiterType>,
}

fn multipart_body<'a, 'b>(
    boundary: &'b ByteStr,
    outer_boundary: Option<&'b ByteStr>,
) -> impl Parser<&'a [u8], MultipartBodyResult<'a>, EmailError<'a>> + 'b
where
    'a: 'b,
{
    let preamble_bound = {
        let mut buf = String::new();
        buf.push_str("^--");
        regex_syntax::escape_into(
            unsafe { std::str::from_utf8_unchecked(&boundary.0) },
            &mut buf,
        );
        buf.push_str(r"\r$\n");
        RegexBuilder::new(&buf).multi_line(true).build().unwrap()
    };

    let epilogue_bound = outer_boundary.map(|ob| {
        let mut buf = String::new();
        buf.push_str("\r\n^");
        regex_syntax::escape_into(unsafe { std::str::from_utf8_unchecked(&ob.0) }, &mut buf);
        buf.push_str(r"(--)?\r$\n");
        RegexBuilder::new(&buf).multi_line(true).build().unwrap()
    });

    move |input| {
        let (preamble_end, main_start) = match preamble_bound.find(input) {
            None => todo!(),
            Some(m) => (m.start(), m.end()),
        };

        let preamble = &input[..preamble_end];
        let mut parts = vec![];
        let mut i = &input[main_start..];
        loop {
            let (
                i2,
                MessageResult {
                    message,
                    ended_with_delim,
                },
            ) = message_inner(Some(boundary)).parse(i)?;
            i = i2;
            parts.push(message);
            if ended_with_delim.unwrap() == DelimiterType::EndOfContainer {
                break;
            }
        }
        let (epilogue_end, overall_end, ended_with_delim) =
            if let Some(epilogue_bound) = &epilogue_bound {
                match epilogue_bound.captures(i) {
                    None => todo!(),
                    Some(c) => {
                        let m = c.get(0).unwrap();
                        (
                            m.start(),
                            m.end(),
                            Some(if c.get(1).is_some() {
                                DelimiterType::EndOfContainer
                            } else {
                                DelimiterType::BetweenParts
                            }),
                        )
                    }
                }
            } else {
                (i.len(), i.len(), None)
            };
        let epilogue = &i[..epilogue_end];
        i = &i[overall_end..];
        Ok((
            i,
            MultipartBodyResult {
                preamble,
                parts,
                epilogue,
                ended_with_delim,
            },
        ))
    }
}

pub struct BodyResult<'a> {
    body: Body<'a>,
    ended_with_delim: Option<DelimiterType>,
}

pub fn body<'a, 'b>(
    // ct: Option<&'b ContentType<'a>>,
    boundary: Option<&'b ByteStr>,
    outer_boundary: Option<&'b ByteStr>,
) -> impl Parser<&'a [u8], BodyResult<'a>, EmailError<'a>> + 'b
where
    'a: 'b,
{
    move |input: &'a [u8]| -> IResult<&'a [u8], BodyResult<'a>, EmailError<'a>> {
        match boundary {
            Some(boundary) => nom::Parser::into(multipart_body(boundary, outer_boundary).map(
                |MultipartBodyResult {
                     preamble,
                     parts,
                     epilogue,
                     ended_with_delim,
                 }| BodyResult {
                    body: Body::Multipart {
                        preamble,
                        parts,
                        epilogue,
                    },
                    ended_with_delim,
                },
            ))
            .parse(input),
            _ => nom::Parser::into(simple_body(outer_boundary).map(
                |SimpleBodyResult {
                     data,
                     lines,
                     ended_with_delim,
                 }| {
                    BodyResult {
                        body: Body::Simple { data, lines },
                        ended_with_delim,
                    }
                },
            ))
            .parse(input),
        }
    }
}

#[derive(Debug)]
struct MessageResult<'a> {
    message: Message<'a>,
    ended_with_delim: Option<DelimiterType>,
}

pub fn message<'a>() -> impl Parser<&'a [u8], Message<'a>, EmailError<'a>> {
    move |input| {
        let (
            i,
            MessageResult {
                message,
                ended_with_delim,
            },
        ) = message_inner(None).parse(input)?;
        assert!(ended_with_delim.is_none());
        Ok((i, message))
    }
}

fn message_inner<'a, 'b>(
    outer_boundary: Option<&'b ByteStr>,
) -> impl Parser<&'a [u8], MessageResult<'a>, EmailError<'a>> + 'b
where
    'a: 'b,
{
    use crate::HeaderField;
    move |input| {
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
                                    return Err(nom::Err::Error(
                                        EmailError::ContentTypeWithoutBoundary,
                                    ))
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

        let (
            i,
            BodyResult {
                body,
                ended_with_delim,
            },
        ) = nom::Parser::into(body(boundary, outer_boundary)).parse(i)?;
        Ok((
            i,
            MessageResult {
                message: Message::new(hfs, ctype_idx, body, input.len()),
                ended_with_delim,
            },
        ))
    }
}
