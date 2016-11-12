extern crate xml;

fn main() {
    let mut p = xml::Parser::new();
    let mut e = xml::ElementBuilder::new();

    let s1 = "<body xmlns='http://jabber.org/protocol/httpbind' ";
    let s2 = "xmlns:xmpp='urn:xmpp:xbosh' ";
    let s3 = "xml:lang='en' ";
    let s4 = "xmpp:restart='true'/>";
    p.feed_str(s1);
    println!("s1: {:?}\n", p);
    p.feed_str(s2);
    println!("s2: {:?}\n", p);
    p.feed_str(s3);
    println!("s3: {:?}\n", p);
    p.feed_str(s4);
    println!("s4: {:?}\n", p);
    for elem in p.filter_map(|x| e.handle_event(x)) {
        match elem {
            Ok(e) => println!("{:?}", e),
            Err(e) => println!("{}", e),
        }
    }
}
