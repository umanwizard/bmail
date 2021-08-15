use std::borrow::Cow;

use nom::character::complete::crlf;

use charset::Charset;
use nom::combinator::all_consuming;
use nom::combinator::consumed;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::error::VerboseError;
use nom::multi::fold_many0;
use nom::multi::fold_many_m_n;
use nom::sequence::preceded;
use nom::sequence::terminated;
use nom::sequence::tuple;
use nom::IResult;
use nom::Parser;
use regex::bytes::RegexBuilder;

use super::header::header_field;
use super::satisfy_byte;

use crate::error::EmailError;
use crate::headers::mime::{ContentTransferEncoding, ContentType};
use crate::headers::HeaderFieldInner;
use crate::Body;
use crate::ByteStr;
use crate::Message;

fn is_non_crlf(ch: u8) -> bool {
    ch != b'\r' && ch != b'\n'
}

fn text998(input: &[u8]) -> IResult<&[u8], &[u8], VerboseError<&[u8]>> {
    recognize(fold_many_m_n(
        0,
        998,
        satisfy_byte(is_non_crlf),
        (),
        |(), _ch| (),
    ))(input)
}

#[derive(Debug, Eq, PartialEq)]
struct SimpleBodyResult<'a> {
    pub data: &'a [u8],
    pub lines: Vec<&'a [u8]>,
}

fn cte_decode<'a>(
    encoding: Option<ContentTransferEncoding>,
) -> impl Parser<&'a [u8], Vec<u8>, EmailError<'a>> {
    move |input: &'a [u8]| {
        let mut collected_input = vec![];
        let mut i = input;
        let should_join = match encoding {
            Some(ContentTransferEncoding::Base64) => true,
            _ => false,
        };
        while !i.is_empty() {
            let (i2, (line, crlf)) =
                nom::Parser::into(consumed(preceded(text998, opt(crlf)))).parse(i)?;
            i = i2;
            if crlf.is_none() && !i.is_empty() {
                eprint!(
                    "Line too long ({}): {}",
                    line.len(),
                    String::from_utf8_lossy(line)
                );
                return Err(nom::Err::Error(EmailError::LineTooLong));
            }
            let line = if should_join && crlf.is_some() {
                &line[..line.len() - 2]
            } else {
                line
            };
            collected_input.extend_from_slice(line);
        }
        let mut collected_input = Some(collected_input);
        let decoded = encoding
            .map(|encoding| encoding.decode(collected_input.take().unwrap()))
            .unwrap_or_else(|| Ok(collected_input.take().unwrap()))
            .map_err(|e| nom::Err::Error(EmailError::BodyDecode(e)))?;
        Ok((i, decoded))
    }
}
fn text_body<'a>(
    encoding: Option<ContentTransferEncoding>,
    charset: Option<Charset>,
) -> impl Parser<&'a [u8], String, EmailError<'a>> {
    move |input| {
        let (i, transfer_decoded): (&[u8], Vec<u8>) =
            nom::Parser::into(cte_decode(encoding)).parse(input)?;
        let text_decoded = match charset {
            // [RFC] default to utf8 instead of the RFCically correct US-ASCII.
            None => String::from_utf8_lossy(&transfer_decoded),
            Some(charset) => charset.decode(&transfer_decoded).0,
        };
        let out = match text_decoded {
            Cow::Borrowed(b) if std::ptr::eq(b.as_bytes(), transfer_decoded.as_slice()) => unsafe {
                String::from_utf8_unchecked(transfer_decoded)
            },
            _ => text_decoded.into_owned(),
        };
        Ok((i, out))
    }
}
struct MultipartBodyResult<'a> {
    preamble: &'a [u8],
    parts: Vec<Message<'a>>,
    epilogue: &'a [u8],
}

fn multipart_body<'a, 'b>(
    boundary: &'b str,
) -> impl Parser<&'a [u8], MultipartBodyResult<'a>, EmailError<'a>> + 'b
where
    'a: 'b,
{
    println!("Boundary: {}", boundary);
    let preamble_bound = {
        let mut buf = String::new();
        buf.push_str("^--");
        regex_syntax::escape_into(boundary, &mut buf);
        buf.push_str(r"\r$\n");
        println!("preamble_bound: {}", buf);
        RegexBuilder::new(&buf).multi_line(true).build().unwrap()
    };

    let inner_bound = {
        let mut buf = String::new();
        buf.push_str("\r\n^--");
        regex_syntax::escape_into(boundary, &mut buf);
        buf.push_str(r"(--)?\r$\n");
        RegexBuilder::new(&buf).multi_line(true).build().unwrap()
    };

    move |input| {
        let (preamble_end, main_start) = match preamble_bound.find(input) {
            None => todo!(),
            Some(m) => (m.start(), m.end()),
        };

        let preamble = &input[..preamble_end];
        let mut parts = vec![];
        let mut i = &input[main_start..];

        loop {
            let (inner_end, next_start, is_done) = match inner_bound.captures(i) {
                None => todo!(),
                Some(c) => {
                    let m = c.get(0).unwrap();
                    (m.start(), m.end(), c.get(1).is_some())
                }
            };
            let i_inner = &i[0..inner_end];
            let (_, part) = all_consuming(message()).parse(i_inner)?;
            parts.push(part);
            i = &i[next_start..];
            if is_done {
                break;
            }
        }
        let epilogue = i;
        i = &i[i.len()..];
        Ok((
            i,
            MultipartBodyResult {
                preamble,
                parts,
                epilogue,
            },
        ))
    }
}

#[derive(Copy, Clone, Debug)]
pub enum MimeParseControl<'a> {
    SimpleText {
        encoding: Option<ContentTransferEncoding>,
        charset: Option<Charset>,
    },
    SimpleBinary {
        encoding: Option<ContentTransferEncoding>,
    },
    Multipart {
        boundary: &'a str,
    },
}

pub fn body<'a, 'b>(
    // ct: Option<&'b ContentType<'a>>,
    mime: MimeParseControl<'b>,
) -> impl Parser<&'a [u8], Body<'a>, EmailError<'a>> + 'b
where
    'a: 'b,
{
    move |input: &'a [u8]| -> IResult<&'a [u8], Body<'a>, EmailError<'a>> {
        match mime {
            MimeParseControl::Multipart { boundary } => {
                nom::Parser::into(multipart_body(boundary).map(
                    |MultipartBodyResult {
                         preamble,
                         parts,
                         epilogue,
                     }| Body::Multipart {
                        preamble,
                        parts,
                        epilogue,
                    },
                ))
                .parse(input)
            }
            MimeParseControl::SimpleText { encoding, charset } => {
                nom::Parser::into(text_body(encoding, charset).map(Body::SimpleText)).parse(input)
            }
            MimeParseControl::SimpleBinary { encoding } => {
                nom::Parser::into(cte_decode(encoding).map(Body::SimpleBinary)).parse(input)
            }
        }
    }
}

pub fn message<'a, 'b>() -> impl Parser<&'a [u8], Message<'a>, EmailError<'a>> + 'b
where
    'a: 'b,
{
    use crate::HeaderField;
    move |input| {
        let (i, (hfs, ctype_idx, cte_idx, _i)): (_, (Vec<HeaderField<'a>>, _, _, _)) =
            terminated(
                fold_many0(
                    header_field,
                    (vec![], None, None, 0),
                    |(mut hfs, ctype_idx, cte_idx, i), hf| {
                        let ctype_idx = ctype_idx.or_else(|| {
                            if let HeaderFieldInner::ContentType(_) = &hf.inner() {
                                Some(i)
                            } else {
                                None
                            }
                        });
                        let cte_idx = cte_idx.or_else(|| {
                            if let HeaderFieldInner::ContentTransferEncoding(_) = &hf.inner() {
                                Some(i)
                            } else {
                                None
                            }
                        });
                        hfs.push(hf);
                        (hfs, ctype_idx, cte_idx, i + 1)
                    },
                ),
                crlf,
            )(input)?;
        let (boundary, charset, is_text) = match ctype_idx {
            Some(ctype_idx) => match hfs[ctype_idx].inner() {
                HeaderFieldInner::ContentType(ContentType {
                    r#type, parameters, ..
                }) => {
                    if r#type.0.eq_ignore_ascii_case(b"multipart") {
                        (
                            Some(match parameters.get("boundary") {
                                Some(boundary) => boundary,
                                None => {
                                    return Err(nom::Err::Error(
                                        EmailError::ContentTypeWithoutBoundary,
                                    ))
                                }
                            }),
                            None,
                            false,
                        )
                    } else if r#type.0.eq_ignore_ascii_case(b"text") {
                        (
                            None,
                            parameters
                                .get("charset")
                                .and_then(|s| Charset::for_label(s.as_bytes())),
                            true,
                        )
                    } else {
                        (None, None, false)
                    }
                }
                _ => unreachable!(),
            },
            None => (None, None, true),
        };

        let cte = cte_idx.map(|cte_idx| match hfs[cte_idx].inner() {
            HeaderFieldInner::ContentTransferEncoding(cte) => *cte,
            _ => unreachable!(),
        });

        let mime_ctl = match (&boundary, charset, cte, is_text) {
            (Some(boundary), _, _, true) => unreachable!(),
            (Some(boundary), _, Some(cte), false) if !cte.is_trivial() => {
                return Err(nom::Err::Error(EmailError::MultipartWithNontrivialCte));
            }
            (Some(boundary), _, _, false) => MimeParseControl::Multipart { boundary },
            (None, charset, encoding, true) => MimeParseControl::SimpleText { encoding, charset },
            (None, Some(charset), _, false) => unreachable!(),
            (None, None, encoding, false) => MimeParseControl::SimpleBinary { encoding },
        };

        let (i, body) = nom::Parser::into(body(mime_ctl)).parse(i)?;
        Ok((i, Message::new(hfs, ctype_idx, body, input.len())))
    }
}
