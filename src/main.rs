use bmail::headers::mime::ContentType;
use bmail::headers::HeaderFieldInner;
use bmail::parse::email::message;

use std::env;

fn main() {
    let args = env::args();

    for f in args.skip(1) {
        println!("{}", f);
        let data = std::fs::read(f).unwrap();
        let (_, message) = nom::combinator::complete(message())(&data).unwrap();
        println!("{:?}", message);
    }
}
