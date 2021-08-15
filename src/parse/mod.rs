use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_while;
use nom::bytes::complete::take_while1;
use nom::character::complete::crlf;

use nom::combinator::map;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::value;
use nom::error::Error;
use nom::error::ErrorKind;
use nom::error::ParseError;
use nom::error::VerboseError;
use nom::multi::fold_many0;

use nom::multi::many0;
use nom::multi::many0_count;
use nom::multi::many1;
use nom::multi::many1_count;
use nom::sequence::tuple;
use nom::Err;
use nom::IResult;
use nom::Parser;

use crate::{ByteStr, ByteString};

pub mod address;
pub mod date_time;
pub mod email;
pub mod header;
pub mod mime;

pub(crate) fn is_wsp(ch: u8) -> bool {
    ch == b' ' || ch == b'\t'
}

fn fws_inner(input: &[u8]) -> IResult<&[u8], (), VerboseError<&[u8]>> {
    let modern_fws = alt((
        value((), tuple((take_while(is_wsp), crlf, take_while1(is_wsp)))),
        value((), take_while1(is_wsp)),
    )); //(input)
        // [RFC] This seems dubious. The RFC implies that obs-fws _must_
        // start with at least one whitespace -- but modern FWS doesn't have to.
        // This precludes multi-line runs of FWS that don't begin with a (horizontal)
        // whitespace character; for example:
        // Subject:
        // ___
        // ___Hello!
        //
        // Is this really what the RFC authors intended?
    let obs_fws = value(
        (),
        tuple((
            take_while1(is_wsp),
            many0_count(tuple((crlf, take_while1(is_wsp)))),
        )),
    );

    alt((obs_fws, modern_fws))(input)
}
/// Recognize folding white space - semantically the newlines are ignored;
/// if you care about the semantic value, call fws_semantic.
pub fn fws(input: &[u8]) -> IResult<&[u8], (), VerboseError<&[u8]>> {
    fws_inner(input)
}

pub fn fws_semantic(input: &[u8]) -> IResult<&[u8], Vec<u8>, VerboseError<&[u8]>> {
    let (i, recognized) = recognize(fws_inner).parse(input)?;
    let mut ret = vec![];
    for ch in recognized.iter().cloned() {
        if ch != b'\r' && ch != b'\n' {
            ret.push(ch);
        }
    }
    Ok((i, ret))
}

fn satisfy_byte<F>(cond: F) -> impl Fn(&[u8]) -> IResult<&[u8], u8, VerboseError<&[u8]>>
where
    F: Fn(u8) -> bool,
{
    move |input| {
        if input.is_empty() {
            Err(Err::Error(VerboseError::from_error_kind(
                input,
                ErrorKind::Eof,
            )))
        } else {
            let ch = input[0];
            if cond(ch) {
                Ok((&input[1..], input[0]))
            } else {
                Err(Err::Error(VerboseError::from_error_kind(
                    input,
                    ErrorKind::Satisfy,
                )))
            }
        }
    }
}

pub fn is_vchar(ch: u8) -> bool {
    0x21 <= ch && ch <= 0x7e
}

fn is_quotable(ch: u8) -> bool {
    is_vchar(ch) || is_wsp(ch)
}

pub fn quoted_pair(input: &[u8]) -> IResult<&[u8], u8, VerboseError<&[u8]>> {
    let (i, (_backslash, ch)) = tuple((tag(b"\\"), satisfy_byte(is_quotable)))(input)?;
    Ok((i, ch))
}

fn is_ctext(ch: u8) -> bool {
    (33 <= ch && ch <= 39) || (42 <= ch && ch <= 91) || (93 <= ch && ch <= 126)
}

fn ccontent(input: &[u8]) -> IResult<&[u8], (), VerboseError<&[u8]>> {
    alt((
        value((), satisfy_byte(is_ctext)),
        value((), quoted_pair),
        comment,
    ))(input)
}

fn comment(input: &[u8]) -> IResult<&[u8], (), VerboseError<&[u8]>> {
    value(
        (),
        tuple((
            tag(b"("),
            many0_count(tuple((opt(fws), ccontent))),
            opt(fws),
            tag(b")"),
        )),
    )(input)
}

fn is_atext(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || b"!#$%&'*+-/=?^_`{|}~".iter().any(|ch2| *ch2 == ch)
}

pub fn is_special(ch: u8) -> bool {
    b"()<>[]:;@\\,.\"".iter().any(|ch2| *ch2 == ch)
}

pub fn atom(input: &[u8]) -> IResult<&[u8], &ByteStr, VerboseError<&[u8]>> {
    map(
        tuple((opt(cfws), take_while1(is_atext), opt(cfws))),
        |(_, the_atom, _)| ByteStr::from_slice(the_atom),
    )(input)
}

fn dot_atom_text(input: &[u8]) -> IResult<&[u8], &ByteStr, VerboseError<&[u8]>> {
    // dot-atom-text   =   1*atext *("." 1*atext)
    map(
        recognize(tuple((
            take_while1(is_atext),
            many0_count(tuple((tag(b"."), take_while1(is_atext)))),
        ))),
        ByteStr::from_slice,
    )(input)
}

pub fn dot_atom(input: &[u8]) -> IResult<&[u8], &ByteStr, VerboseError<&[u8]>> {
    map(
        tuple((opt(cfws), dot_atom_text, opt(cfws))),
        |(_, the_atom, _)| the_atom,
    )(input)
}

pub fn cfws(input: &[u8]) -> IResult<&[u8], (), VerboseError<&[u8]>> {
    alt((
        value(
            (),
            tuple((many1_count(tuple((opt(fws), comment))), opt(fws))),
        ),
        fws,
    ))(input)
}

fn is_qtext(ch: u8) -> bool {
    ch == 33 || (35 <= ch && ch <= 91) || (93 <= ch && ch <= 126)
}

fn qcontent(input: &[u8]) -> IResult<&[u8], u8, VerboseError<&[u8]>> {
    alt((satisfy_byte(is_qtext), quoted_pair))(input)
}

// TODO - Cow here when possible, rather than always allocating?
pub fn quoted_string(input: &[u8]) -> IResult<&[u8], ByteString, VerboseError<&[u8]>> {
    map(
        tuple((
            opt(cfws),
            tag(b"\""),
            fold_many0(
                tuple((opt(fws_semantic), qcontent)),
                vec![],
                |mut s, (maybe_fws, ch)| {
                    if let Some(fws) = maybe_fws {
                        s.extend_from_slice(&fws);
                    }
                    s.push(ch);
                    s
                },
            ),
            opt(fws),
            tag(b"\""),
            opt(cfws),
        )),
        |(_, _, s, _, _, _)| ByteString(s),
    )(input)
}

// TODO - Cow when possible?
fn word(input: &[u8]) -> IResult<&[u8], ByteString, VerboseError<&[u8]>> {
    alt((map(atom, ToOwned::to_owned), quoted_string))(input)
}

// TODO - Cow when possible?
pub fn phrase(i: &[u8]) -> IResult<&[u8], Vec<ByteString>, VerboseError<&[u8]>> {
    let modern_phrase = many1(word);
    let obs_phrase = |i| {
        let (i, first) = word(i)?;
        let words = vec![first];
        fold_many0(
            alt((
                map(word, Some),
                map(tag(b"."), |_dot| Some(ByteStr::from_slice(b".").to_owned())),
                map(cfws, |_| None),
            )),
            words,
            |mut words, maybe_word| {
                if let Some(word) = maybe_word {
                    words.push(word);
                }
                words
            },
        )(i)
    };
    alt((obs_phrase, modern_phrase))(i)
}

#[test]
fn test_phrase() {
    let input = b"=?utf-8?Q?Register.ly?=";

    use nom::combinator::all_consuming;
    let x = all_consuming(phrase)(input).unwrap();
    println!("{:?}", x);
}

#[test]
pub fn test_multiword_phrase() {
    use nom::combinator::complete;

    let test = b"Brennan Vincent";

    let x = complete(phrase)(test).unwrap();
    eprintln!("{:?}", x);
}

// TODO - Cow when possible?
pub fn unstructured(input: &[u8]) -> IResult<&[u8], ByteString, VerboseError<&[u8]>> {
    let (i, o) = fold_many0(
        tuple((opt(fws), satisfy_byte(is_vchar))),
        vec![],
        |mut s, (maybe_fws, ch)| {
            if let Some(()) = maybe_fws {
                s.push(b' ');
            }
            s.push(ch);
            s
        },
    )(input)?;
    map(
        fold_many0(satisfy_byte(is_wsp), o, |mut s, ch| {
            s.push(ch);
            s
        }),
        ByteString,
    )(i)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_fws() {
        let (i, ()) = super::fws(b"    \r\n   hi!").unwrap();
        assert_eq!(i, b"hi!");
    }
}
