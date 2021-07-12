use bmail::parse::email::message;
use std::env;

fn main() {
    let args = env::args();

    for f in args.skip(1) {
        eprintln!("f: {}", f);
        let data = std::fs::read(f).unwrap();
        let (_, message) = nom::combinator::complete(message)(&data).unwrap();
        eprintln!("{:?}", message);
    }
}
