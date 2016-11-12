extern crate xml;

fn main() {
    let mut p = xml::Parser::new();
    let mut e = xml::ElementBuilder::new();

    p.feed_str("<a href='//example.com'/>");
    for elem in p.filter_map(|x| e.handle_event(x)) {
        match elem {
            Ok(e) => println!("{:?}", e),
            Err(e) => println!("{}", e),
        }
    }
    p.feed_str("<a");
    for elem in p.filter_map(|x| e.handle_event(x)) {
        match elem {
            Ok(e) => println!("{:?}", e),
            Err(e) => println!("{}", e),
        }
    }
    p.feed_str("/>");
    for elem in p.filter_map(|x| e.handle_event(x)) {
        match elem {
            Ok(e) => println!("{:?}", e),
            Err(e) => println!("{}", e),
        }
    }
    p.feed_str("<stream>");
    p.feed_str("<inner-element/>");
    for elem in p.filter_map(|x| e.handle_event(x)) {
        match elem {
            Ok(e) => println!("{:?}", e),
            Err(e) => println!("{}", e),
        }
    }
    p.feed_str("</stream>");
    for elem in p.filter_map(|x| e.handle_event(x)) {
        match elem {
            Ok(e) => println!("{:?}", e),
            Err(e) => println!("{}", e),
        }
    }
}
