// RustyXML
// Copyright (c) 2013, 2014 Florian Zeitz
//
// This project is MIT licensed.
// Please see the COPYING file for more information.
//
// The parser herein is derived from OFXMLParser as included with
// ObjFW, Copyright (c) 2008-2013 Jonathan Schleifer.
// Permission to license this derived work under MIT license has been granted by ObjFW's author.

use super::{unescape, Attribute, Event, PI, StartTag, EndTag, Characters, CDATA, Comment};
use collections::HashMap;
use std::mem;

#[deriving(Eq, Show)]
/// If an error occurs while parsing some XML, this is the structure which is
/// returned
pub struct Error {
    /// The line number at which the error occurred
    pub line: uint,
    /// The column number at which the error occurred
    pub col: uint,
    /// A message describing the type of the error
    pub msg: &'static str
}

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

/// A streaming XML parser
pub struct Parser {
    line: uint,
    col: uint,
    buf: StrBuf,
    name: StrBuf,
    prefix: Option<StrBuf>,
    namespaces: Vec<HashMap<StrBuf, StrBuf>>,
    attr_name: StrBuf,
    attr_prefix: Option<StrBuf>,
    attributes: Vec<Attribute>,
    delim: Option<char>,
    st: State,
    level: uint
}

impl Parser {
    /// Returns a new `Parser`
    pub fn new() -> Parser {
        let mut p = Parser {
            line: 1,
            col: 0,
            buf: StrBuf::new(),
            name: StrBuf::new(),
            prefix: None,
            namespaces: vec!(HashMap::with_capacity(2)),
            attr_name: StrBuf::new(),
            attr_prefix: None,
            attributes: Vec::new(),
            delim: None,
            st: OutsideTag,
            level: 0
        };
        {
            let x = p.namespaces.get_mut(0);
            x.swap("xml".to_strbuf(), "http://www.w3.org/XML/1998/namespace".to_strbuf());
            x.swap("xmlns".to_strbuf(), "http://www.w3.org/2000/xmlns/".to_strbuf());
        }
        p
    }

    /**
     * Parses the string `data`.
     * The callback `cb` is called for each `Event`, or `Error` generated while parsing
     * the string.
     *
     * ~~~
     * let mut p = Parser::new();
     * do p.parse_str("<a href='http://rust-lang.org'>Rust</a>") |event| {
     *     match event {
     *        [...]
     *     }
     * }
     * ~~~
     */
    pub fn parse_str(&mut self, data: &str, cb: |Result<Event, Error>|) {
        for c in data.chars() {
            if c == '\n' {
                self.line += 1u;
                self.col = 0u;
            } else {
                self.col += 1u;
            }

            match self.parse_character(c) {
                Ok(None) => continue,
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

#[inline]
fn parse_qname(qname: &str) -> (Option<StrBuf>, StrBuf) {
    match qname.find(':') {
        None => {
            (None, qname.to_strbuf())
        },
        Some(i) => {
            (Some(qname.slice_to(i).to_strbuf()), qname.slice_from(i+1).to_strbuf())
        }
    }
}

impl Parser {
    fn namespace_for_prefix(&self, prefix: &StrBuf) -> Option<StrBuf> {
        for ns in self.namespaces.as_slice().iter().rev() {
            match ns.find(prefix) {
                None => continue,
                Some(namespace) => {
                    if namespace.len() == 0 {
                        return None;
                    } else {
                        return Some(namespace.clone());
                    }
                }
            }
        }
        None
    }

    fn error(&self, msg: &'static str) -> Result<Option<Event>, Error> {
        Err(Error { line: self.line, col: self.col, msg: msg })
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
                let buf = match unescape(self.buf.as_slice()) {
                    Ok(unescaped) => unescaped,
                    Err(_) => return self.error("Found invalid entity")
                };
                self.buf.truncate(0);
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
                let _ = self.buf.pop_char();
                let buf = mem::replace(&mut self.buf, StrBuf::new());
                return Ok(Some(PI(buf)));
            }
            _ => self.buf.push_char(c)
        }
        Ok(None)
    }

    fn in_tag_name(&mut self, c: char) -> Result<Option<Event>, Error> {
        fn set_name(p: &mut Parser) {
            let (prefix, name) = parse_qname(p.buf.as_slice());
            p.prefix = prefix;
            p.name = name;
            p.buf.truncate(0);
        };

        match c {
            '/'
            | '>' => {
                set_name(self);
                let prefix = self.prefix.take();
                let ns = match prefix {
                    None => self.namespace_for_prefix(&StrBuf::new()),
                    Some(ref pre) => {
                        self.namespace_for_prefix(pre).or_else(|| {
                            fail!("Unbound prefix: '{}'", *pre)
                        })
                    }
                };

                self.namespaces.push(HashMap::new());
                self.st = if c == '/' {
                    ExpectClose
                } else {
                    OutsideTag
                };

                return Ok(Some(StartTag(StartTag {
                    name: self.name.clone(),
                    ns: ns,
                    prefix: prefix,
                    attributes: Vec::new()
                })));
            }
            ' '
            | '\t'
            | '\r'
            | '\n' => {
                self.namespaces.push(HashMap::new());
                set_name(self);
                self.st = InTag;
            }
            _ => self.buf.push_char(c)
        }
        Ok(None)
    }

    fn in_close_tag_name(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            ' '
            | '\t'
            | '\r'
            | '\n'
            | '>' => {
                let (prefix, name) = parse_qname(self.buf.as_slice());
                self.buf.truncate(0);

                let ns = match prefix {
                    None => self.namespace_for_prefix(&StrBuf::new()),
                    Some(ref pre) => {
                        self.namespace_for_prefix(pre).or_else(|| {
                            fail!("Unbound prefix: '{}'", *pre)
                        })
                    }
                };

                self.namespaces.pop();
                self.st = if c == '>' {
                    OutsideTag
                } else {
                    ExpectSpaceOrClose
                };

                Ok(Some(EndTag(EndTag { name: name, ns: ns, prefix: prefix })))
            }
            _ => {
                self.buf.push_char(c);
                Ok(None)
            }
        }
    }

    fn in_tag(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '/'
            | '>' => {
                let name = mem::replace(&mut self.name, StrBuf::new());
                let mut attributes = mem::replace(&mut self.attributes, Vec::new());
                let prefix = self.prefix.take();
                let ns = match prefix {
                    None => self.namespace_for_prefix(&StrBuf::new()),
                    Some(ref pre) => {
                        self.namespace_for_prefix(pre).or_else(|| {
                            fail!("Unbound prefix: '{}'", *pre)
                        })
                    }
                };

                for attr in attributes.mut_iter() {
                    attr.ns.mutate( |ref pre| {
                        self.namespace_for_prefix(pre).unwrap_or_else( || {
                            fail!("Unbound prefix: '{}'", *pre)
                        })
                    });
                }

                self.st = if c == '/' {
                    ExpectClose
                } else {
                    self.prefix = None;
                    OutsideTag
                };

                return Ok(Some(StartTag(StartTag {
                    name: name,
                    ns: ns,
                    prefix: prefix,
                    attributes: attributes
                })));
            }
            ' '
            | '\t'
            | '\r'
            | '\n' => (),
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

                let (prefix, name) = parse_qname(self.buf.as_slice());
                self.attr_prefix = prefix;
                self.attr_name = name;

                self.buf.truncate(0);
                self.st = ExpectDelimiter;
            }
            ' '
            | '\t'
            | '\r'
            | '\n' => self.level = 1,
            _ if self.level == 0 => self.buf.push_char(c),
            _ => return self.error("Space occured in attribute name")
        }
        Ok(None)
    }

    fn in_attr_value(&mut self, c: char) -> Result<Option<Event>, Error> {
        if c == self.delim.expect("In attribute value, but no delimiter set") {
            self.delim = None;
            self.st = InTag;
            let name = mem::replace(&mut self.attr_name, StrBuf::new());
            let value = match unescape(self.buf.as_slice()) {
                Ok(unescaped) => unescaped,
                Err(_) => return self.error("Found invalid entity")
            };
            self.buf.truncate(0);
            let prefix = self.attr_prefix.take();

            let last = self.namespaces.mut_last().unwrap();
            match prefix {
                None if name.as_slice() == "xmlns" => {
                    last.swap(StrBuf::new(), value.clone());
                }
                Some(ref prefix) if prefix.as_slice() == "xmlns" => {
                    last.swap(name.clone(), value.clone());
                }
                _ => ()
            }

            self.attributes.push(Attribute { name: name, ns: prefix, value: value });
        } else {
            self.buf.push_char(c);
        }
        Ok(None)
    }

    fn expect_delimiter(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '"'
            | '\'' => {
                self.delim = Some(c);
                self.st = InAttrValue;
            }
            ' '
            | '\t'
            | '\r'
            | '\n' => (),
            _ => return self.error("Attribute value not enclosed in ' or \"")
        }
        Ok(None)
    }

    fn expect_close(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            '>' => {
                self.st = OutsideTag;
                let name = mem::replace(&mut self.name, StrBuf::new());
                let prefix = self.prefix.take();
                let ns = match prefix {
                    None => self.namespace_for_prefix(&StrBuf::new()),
                    Some(ref pre) => {
                        self.namespace_for_prefix(pre).or_else(|| {
                            fail!("Unbound prefix: '{}'", *pre)
                        })
                    }
                };
                self.namespaces.pop();
                Ok(Some(EndTag(EndTag { name: name, ns: ns, prefix: prefix })))
            }
            _ => self.error("Expected '>' to close tag")
       }
    }

    fn expect_space_or_close(&mut self, c: char) -> Result<Option<Event>, Error> {
        match c {
            ' '
            | '\t'
            | '\r'
            | '\n' => Ok(None),
            '>' => {
                self.st = OutsideTag;
                Ok(None)
            }
            _ => self.error("Expected '>' to close tag, or LWS")
       }
    }

    fn in_exclamation_mark(&mut self, c: char) -> Result<Option<Event>, Error> {
        self.st = match c {
            '-' => InCommentOpening,
            '[' => InCDATAOpening,
            'D' => InDoctype,
            _ => return self.error("Malformed XML")
        };
        Ok(None)
    }

    fn in_CDATA_opening(&mut self, c: char) -> Result<Option<Event>, Error> {
        static CDATA_PATTERN: [char, ..6] = ['C', 'D', 'A', 'T', 'A', '['];
        if c == CDATA_PATTERN[self.level] {
            self.level += 1;
        } else {
            return self.error("Invalid CDATA opening sequence")
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
                let len = self.buf.len();
                self.buf.truncate(len - 2);
                let buf = mem::replace(&mut self.buf, StrBuf::new());
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
            self.error("Expected 2nd '-' to start comment")
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
            self.error("Not more than one adjacent '-' allowed in a comment")
        } else {
            self.st = OutsideTag;
            let len = self.buf.len();
            self.buf.truncate(len - 2);
            let buf = mem::replace(&mut self.buf, StrBuf::new());
            Ok(Some(Comment(buf)))
        }
    }

    fn in_doctype(&mut self, c: char) -> Result<Option<Event>, Error> {
        static DOCTYPE_PATTERN: [char, ..6] = ['O', 'C', 'T', 'Y', 'P', 'E'];
        match self.level {
            0..5 => if c == DOCTYPE_PATTERN[self.level] {
                self.level += 1;
            } else {
                return self.error("Invalid DOCTYPE");
            },
            6 => {
                match c {
                    ' '
                    | '\t'
                    | '\r'
                    | '\n' => (),
                    _ => return self.error("Invalid DOCTYPE")
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
