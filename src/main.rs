use bmail::headers::mime::ContentType;
use bmail::headers::HeaderFieldInner;
use bmail::parse::email::message;

use std::env;

fn main() {
    let args = env::args();

    for f in args.skip(1) {
        println!("{}", f);
        let data = std::fs::read(f).unwrap();
        let (_, message) = nom::combinator::complete(message)(&data).unwrap();
        for hf in message.header().iter() {
            if hf.name().0.eq_ignore_ascii_case(b"content-type") {
                let ContentType {
                    r#type,
                    subtype,
                    parameters,
                } = match hf.inner() {
                    HeaderFieldInner::ContentType(ct) => ct,
                    _ => unreachable!(),
                };
                print!("Content-Type: {:?}/{:?}", r#type, subtype);
                for (attribute, value) in parameters.iter() {
                    print!("; {:?}={:?}", attribute, value);
                }
                println!("");
            }
        }
    }
}
