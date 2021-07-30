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

pub fn body<'a>(
    ct: Option<&ContentType>,
) -> impl Parser<&'a [u8], Vec<&'a [u8]>, nom::error::Error<&'a [u8]>> {
    match ct {
        None => separated_list0(crlf, text998).map(|mut v| {
            if v.last() == Some(&&b""[..]) {
                v.pop();
            }
            v
        }),
        Some(_ct) => todo!(),
    }
}

pub fn message(input: &[u8]) -> IResult<&[u8], Message, EmailError> {
    let (i, (hfs, ctype_idx, _i)) = terminated(
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
    let ct = ctype_idx.map(|ctype_idx| match &hfs[ctype_idx].inner() {
        HeaderFieldInner::ContentType(ct) => ct,
        _ => unreachable!(),
    });
    let (i, (body, body_lines)) = nom::Parser::into(consumed(body(ct))).parse(i)?;
    Ok((
        i,
        Message::new(hfs, ctype_idx, body, body_lines, input.len()),
    ))
}
