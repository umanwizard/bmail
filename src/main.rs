use bmail::error::EmailError;
use bmail::headers::mime::ContentType;
use bmail::headers::HeaderFieldInner;
use bmail::parse::email::message;

use nom::error::VerboseError as NomVerboseError;
use nom::Err as NomErr;
use nom::Parser;

use std::env;

fn main() {
    let args = env::args();

    for f in args.skip(1) {
        eprintln!("{}", f);
        let data = std::fs::read(&f).unwrap();
        let (_, message) = match message().parse(&data) {
            Ok(ok) => ok,
            Err(NomErr::Error(e)) => {
                eprintln!("Error: {:?}", e);
                return;
            }
            Err(e) => panic!("{:?}", e),
        };
        //        println!("{:?}", message);
    }
}
