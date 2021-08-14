use bmail::error::EmailError;
use bmail::headers::mime::ContentType;
use bmail::headers::HeaderFieldInner;
use bmail::parse::email::message;

use nom::error::Error as NomError;
use nom::Err as NomErr;

use std::env;

fn main() {
    let args = env::args();

    for f in args.skip(1) {
        println!("{}", f);
        let data = std::fs::read(f).unwrap();
        let (_, message) = match nom::combinator::complete(message())(&data) {
            Ok(ok) => ok,
            Err(NomErr::Error(EmailError::Parse(NomError { input, .. }, _))) => {
                panic!("Error at: {}", String::from_utf8_lossy(input));
            }
            Err(e) => panic!("{:?}", e),
        };
        println!("{:?}", message);
    }
}
