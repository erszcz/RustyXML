use base::*;

// Event based parser
enum State {
    OutsideTag,
    TagOpened,
    InProcessingInstructions,
    InTagName,
    InCloseTagName,
    InTag,
    InAttrName,
    InAttrValue,
    ExpectDelimiter,
    ExpectClose,
    ExpectSpaceOrClose,
    InExclamationMark,
    InCDATAOpening,
    InCDATA,
    InCommentOpening,
    InComment1,
    InComment2,
    InDoctype
}

pub struct Parser {
    priv line: uint,
    priv col: uint,
    priv buf: ~str,
    priv name: ~str,
    priv attrName: ~str,
    priv attributes: ~[Attribute],
    priv delim: char,
    priv st: State,
    priv level: uint
}

pub fn Parser() -> Parser {
    let p = Parser {
        line: 1,
        col: 0,
        buf: ~"",
        name: ~"",
        attrName: ~"",
        attributes: ~[],
        delim: 0 as char,
        st: OutsideTag,
        level: 0
    };
    p
}

impl Parser {
    pub fn parse_str(&mut self, data: &str, cb: &fn(Result<Event, Error>)) {
        for data.iter().advance |c| {
            if c == '\n' {
                self.line += 1u;
                self.col = 0u;
            } else {
                self.col += 1u;
            }

            match self.parse_character(c) {
                Ok(None) => loop,
                Err(e) => {
                    cb(Err(e));
                    return;
                }
                Ok(Some(event)) => {
                    cb(Ok(event));
                }
            }
        }
    }
}

impl Parser {
    fn error(&self, msg: ~str) -> Result<Option<Event>, Error> {
        Err(Error { line: self.line, col: self.col, msg: @msg })
    }

    fn parse_character(&mut self, c: char) -> Result<Option<Event>, Error> {
        // println(fmt!("Now in state: %?", self.st));
        match self.st {
            OutsideTag => self.outside_tag(c),
            TagOpened => self.tag_opened(c),
            InProcessingInstructions => self.in_processing_instructions(c),
            InTagName => self.in_tag_name(c),
            InCloseTagName => self.in_close_tag_name(c),
            InTag => self.in_tag(c),
            InAttrName => self.in_attr_name(c),
            InAttrValue => self.in_attr_value(c),
            ExpectDelimiter => self.expect_delimiter(c),
            ExpectClose => self.expect_close(c),
            ExpectSpaceOrClose => self.expect_space_or_close(c),
            InExclamationMark => self.in_exclamation_mark(c),
            InCDATAOpening => self.in_CDATA_opening(c),
            InCDATA => self.in_CDATA(c),
            InCommentOpening => self.in_comment_opening(c),
            InComment1 => self.in_comment1(c),
            InComment2 => self.in_comment2(c),
            InDoctype => self.in_doctype(c),
        }
    }

    fn outside_tag(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '<' if self.buf.len() > 0 => {
                self.st = TagOpened;
                let buf = unescape(self.buf);
                self.buf = ~"";
                return Ok(Some(Characters(buf)));
            }
            '<' => self.st = TagOpened,
            _ => self.buf.push_char(c)
        }
        Ok(None)
    }

    fn tag_opened(&mut self, c: char) -> Result<Option<Event>, Error> {
        self.st = match c {
            '?' => InProcessingInstructions,
            '!' => InExclamationMark,
            '/' => InCloseTagName,
            _ => {
                self.buf.push_char(c);
                InTagName
            }
        };
        Ok(None)
    }

    fn in_processing_instructions(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '?' => {
                self.level = 1;
                self.buf.push_char(c);
            }
            '>' if self.level == 1 => {
                self.level = 0;
                self.st = OutsideTag;
                let buf = self.buf.slice_chars(0, self.buf.char_len()-1).to_owned();
                self.buf = ~"";
                return Ok(Some(PI(buf)));
            }
            _ => self.buf.push_char(c)
        }
        Ok(None)
    }

    fn in_tag_name(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '/' | '>' => {
                self.st = if c == '/' {
                    ExpectClose
                } else {
                    OutsideTag
                };
                self.name = self.buf.clone();
                self.buf = ~"";
                let name = self.name.clone();
                return Ok(Some(StartTag { name: name, attributes: ~[] }));
            }
            ' ' | '\t' | '\r' | '\n' => {
                self.name = self.buf.clone();
                self.buf = ~"";
                self.st = InTag;
            }
            _ => self.buf.push_char(c)
        }
        Ok(None)
    }

    fn in_close_tag_name(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            ' ' | '\t' | '\r' | '\n' | '>' => {
                let buf = self.buf.clone();
                self.buf = ~"";
                self.st = if c == '>' {
                    OutsideTag
                } else {
                    ExpectSpaceOrClose
                };
                Ok(Some(EndTag { name: buf }))
            }
            _ => {
                self.buf.push_char(c);
                Ok(None)
            }
        }
    }

    fn in_tag(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '/' | '>' => {
                let name = self.name.clone();
                self.st = if c == '/' {
                    ExpectClose
                } else {
                    self.name = ~"";
                    OutsideTag
                };
                let attr = self.attributes.clone();
                self.attributes = ~[];
                return Ok(Some(StartTag { name: name, attributes: attr }));
            }
            ' ' | '\t' | '\r' | '\n' => (),
            _ => {
                self.buf.push_char(c);
                self.st = InAttrName;
            }
        }
        Ok(None)
    }

    fn in_attr_name(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '=' => {
                self.level = 0;
                self.attrName = self.buf.clone();
                self.buf = ~"";
                self.st = ExpectDelimiter;
            }
            ' ' | '\t' | '\r' | '\n' => self.level = 1,
            _ if self.level == 0 => self.buf.push_char(c),
            _ => return self.error(~"Space occured in attribute name")
        }
        Ok(None)
    }

    fn in_attr_value(&mut self, c: char) -> Result<Option<Event>, Error> {
        if c == self.delim {
            self.delim = 0 as char;
            self.st = InTag;
            let name = self.attrName.clone();
            self.attrName = ~"";
            let value = unescape(self.buf);
            self.buf = ~"";
            self.attributes.push(Attribute { name: name, value: value });
        } else {
            self.buf.push_char(c);
        }
        Ok(None)
    }

    fn expect_delimiter(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '"' | '\'' => {
                self.delim = c;
                self.st = InAttrValue;
            }
            ' ' | '\t' | '\r' | '\n' => (),
            _ => return self.error(~"Attribute value not enclosed in ' or \"")
        }
        Ok(None)
    }

    fn expect_close(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '>' => {
                self.st = OutsideTag;
                let name = self.name.clone();
                self.name = ~"";
                Ok(Some(EndTag { name: name }))
            }
            _ => self.error(~"Expected '>' to close tag")
       }
    }

    fn expect_space_or_close(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            ' ' | '\t' | '\r' | '\n' => Ok(None),
            '>' => {
                self.st = OutsideTag;
                Ok(None)
            }
            _ => self.error(~"Expected '>' to close tag, or LWS")
       }
    }

    fn in_exclamation_mark(&mut self, c: char) -> Result<Option<Event>, Error> {
        self.st = match c {
            '-' => InCommentOpening,
            '[' => InCDATAOpening,
            'D' => InDoctype,
            _ => return self.error(~"Malformed XML")
        };
        Ok(None)
    }

    fn in_CDATA_opening(&mut self, c: char) -> Result<Option<Event>, Error> {
        static CDATAPattern: [char, ..6] = ['C', 'D', 'A', 'T', 'A', '['];
        if c == CDATAPattern[self.level] {
            self.level += 1;
        } else {
            return self.error(~"Invalid CDATA opening sequence")
        }

        if self.level == 6 {
            self.level = 0;
            self.st = InCDATA;
        }
        Ok(None)
    }

    fn in_CDATA(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            ']' => {
                self.buf.push_char(c);
                self.level += 1;
            }
            '>' if self.level >= 2 => {
                self.st = OutsideTag;
                self.level = 0;
                let buf = self.buf.slice_chars(0, self.buf.char_len()-2).to_owned();
                self.buf = ~"";
                return Ok(Some(CDATA(buf)))
            }
            _ => {
                self.buf.push_char(c);
                self.level = 0;
            }
        }
        Ok(None)
    }

    fn in_comment_opening(&mut self, c: char) -> Result<Option<Event>, Error> {
        if c == '-' {
            self.st = InComment1;
            self.level = 0;
            Ok(None)
        } else {
            self.error(~"Expected 2nd '-' to start comment")
        }
    }

    fn in_comment1(&mut self, c: char) -> Result<Option<Event>, Error> {
        if c == '-' {
            self.level += 1;
        } else {
            self.level = 0;
        }

        if self.level == 2 {
            self.level = 0;
            self.st = InComment2;
        }

        self.buf.push_char(c);

        Ok(None)
    }

    fn in_comment2(&mut self, c: char) -> Result<Option<Event>, Error> {
        if c != '>' {
            self.error(~"Not more than one adjacent '-' allowed in a comment")
        } else {
            self.st = OutsideTag;
            let buf = self.buf.slice_chars(0, self.buf.char_len()-2).to_owned();
            self.buf = ~"";
            Ok(Some(Comment(buf)))
        }
    }

    fn in_doctype(&mut self, c: char) -> Result<Option<Event>, Error> {
        static DOCTYPEPattern: [char, ..6] = ['O', 'C', 'T', 'Y', 'P', 'E'];
        match self.level {
            0..5 => if c == DOCTYPEPattern[self.level] {
                self.level += 1;
            } else {
                return self.error(~"Invalid DOCTYPE");
            },
            6 => {
                match c {
                    ' ' | '\t' | '\r' | '\n' => (),
                    _ => return self.error(~"Invalid DOCTYPE")
                }
                self.level += 1;
            }
            _ if c == '>' => {
                self.level = 0;
                self.st = OutsideTag;
            }
            _ => ()
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::*;

    #[test]
    fn test_start_tag() {
        let mut p = Parser();
        let mut i = 0;
        do p.parse_str("<a>") |event| {
            i += 1;
            assert_eq!(event, Ok(StartTag { name: ~"a", attributes: ~[] }));
        }
        assert_eq!(i, 1);
    }

    #[test]
    fn test_end_tag() {
        let mut p = Parser();
        let mut i = 0;
        do p.parse_str("</a>") |event| {
            i += 1;
            assert_eq!(event, Ok(EndTag { name: ~"a" }));
        }
        assert_eq!(i, 1);
    }

    #[test]
    fn test_PI() {
        let mut p = Parser();
        let mut i = 0;
        do p.parse_str("<?xml version='1.0' encoding='utf-8'?>") |event| {
            i += 1;
            assert_eq!(event, Ok(PI(~"xml version='1.0' encoding='utf-8'")));
        }
        assert_eq!(i, 1);
    }

    #[test]
    fn test_comment() {
        let mut p = Parser();
        let mut i = 0;
        do p.parse_str("<!--Nothing to see-->") |event| {
            i += 1;
            assert_eq!(event, Ok(Comment(~"Nothing to see")));
        }
        assert_eq!(i, 1);
    }
    #[test]
    fn test_CDATA() {
        let mut p = Parser();
        let mut i = 0;
        do p.parse_str("<![CDATA[<message to='florob@babelmonkeys.de' type='chat'><body>Hello</body></message>]]>") |event| {
            i += 1;
            assert_eq!(event, Ok(CDATA(~"<message to='florob@babelmonkeys.de' type='chat'><body>Hello</body></message>")));
        }
        assert_eq!(i, 1);
    }

    #[test]
    fn test_characters() {
        let mut p = Parser();
        let mut i = 0;
        do p.parse_str("<text>Hello World, it&apos;s a nice day</text>") |event| {
            i += 1;
            if i == 2 {
                assert_eq!(event, Ok(Characters(~"Hello World, it's a nice day")));
            }
        }
        assert_eq!(i, 3);
    }

    #[test]
    fn test_doctype() {
        let mut p = Parser();
        let mut i = 0;
        do p.parse_str("<!DOCTYPE html>") |_| {
            i += 1;
        }
        assert_eq!(i, 0);
    }
}