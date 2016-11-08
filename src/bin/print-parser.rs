extern crate xml;

use xml::{ Parser };

fn main() {
    let mut p = Parser::new();
    p.feed_str("<ala-ma");
    println!("{:?}", p);
}
