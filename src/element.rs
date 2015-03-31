// RustyXML
// Copyright (c) 2013-2015 Florian Zeitz
//
// This project is MIT licensed.
// Please see the COPYING file for more information.

use {escape, Xml};
use element_builder::{BuilderError, ElementBuilder};
use parser::Parser;

use std::fmt;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Clone, PartialEq, Debug)]
/// A struct representing an XML element
pub struct Element {
    /// The element's name
    pub name: String,
    /// The element's namespace
    pub ns: Option<String>,
    /// The element's default namespace
    pub default_ns: Option<String>,
    /// The prefixes set for known namespaces
    pub prefixes: HashMap<String, String>,
    /// The element's attributes
    pub attributes: HashMap<(String, Option<String>), String>,
    /// The element's child `Xml` nodes
    pub children: Vec<Xml>,
}

fn fmt_elem(elem: &Element, parent: Option<&Element>, all_prefixes: &HashMap<String, String>,
            f: &mut fmt::Formatter) -> fmt::Result {
    let mut all_prefixes = all_prefixes.clone();
    all_prefixes.extend(elem.prefixes.iter().map(|(k, v)| (k.clone(), v.clone()) ));

    // Do we need a prefix?
    try!(if elem.ns != elem.default_ns {
        let prefix = all_prefixes.get(elem.ns.as_ref().map(|x| &x[..]).unwrap_or(""))
                                 .expect("No namespace prefix bound");
        write!(f, "<{}:{}", *prefix, elem.name)
    } else {
        write!(f, "<{}", elem.name)
    });

    // Do we need to set the default namespace ?
    if !elem.attributes.iter().any(|(&(ref name, _), _)| *name == "xmlns") {
        match (parent, &elem.default_ns) {
            // No parent, namespace is not empty
            (None, &Some(ref ns)) => try!(write!(f, " xmlns='{}'", *ns)),
            // Parent and child namespace differ
            (Some(parent), ns) if parent.default_ns != *ns => {
                try!(write!(f, " xmlns='{}'", ns.as_ref().map(|x| &x[..]).unwrap_or("")))
            },
            _ => ()
        }
    }

    for (&(ref name, ref ns), value) in &elem.attributes {
        try!(match *ns {
            Some(ref ns) => {
                let prefix = all_prefixes.get(ns).expect("No namespace prefix bound");
                write!(f, " {}:{}='{}'", *prefix, name, escape(&value))
            }
            None => write!(f, " {}='{}'", name, escape(&value))
        });
    }

    if elem.children.len() == 0 {
        write!(f, "/>")
    } else {
        try!(write!(f, ">"));
        for child in &elem.children {
            try!(match *child {
                Xml::ElementNode(ref child) => fmt_elem(child, Some(elem), &all_prefixes, f),
                ref o => fmt::Display::fmt(o, f)
            });
        }
        if elem.ns != elem.default_ns {
            let prefix = all_prefixes.get(elem.ns.as_ref().unwrap())
                                     .expect("No namespace prefix bound");
            write!(f, "</{}:{}>", *prefix, elem.name)
        } else {
            write!(f, "</{}>", elem.name)
        }
    }
}

impl fmt::Display for Element{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_elem(self, None, &HashMap::new(), f)
    }
}

impl Element {
    /// Create a new `Element`, with specified name and namespace.
    /// Attributes are specified as a `Vec` of `(name, namespace, value)` tuples.
    pub fn new(name: String, ns: Option<String>,
               attrs: Vec<(String, Option<String>, String)>) -> Element {
        let attributes: HashMap<_, _> = attrs.into_iter()
                                             .map(|(name, ns, value)| ((name, ns), value))
                                             .collect();
        Element {
            name: name,
            ns: ns.clone(),
            default_ns: ns,
            prefixes: HashMap::new(),
            attributes: attributes,
            children: Vec::new()
        }
    }

    /// Returns the character and CDATA contained in the element.
    pub fn content_str(&self) -> String {
        let mut res = String::new();
        for child in &self.children {
            match *child {
                Xml::ElementNode(ref elem) => res.push_str(&elem.content_str()),
                Xml::CharacterNode(ref data)
                | Xml::CDATANode(ref data) => res.push_str(&data),
                _ => ()
            }
        }
        res
    }

    /// Gets an attribute with the specified name and namespace. When an attribute with the
    /// specified name does not exist `None` is returned.
    pub fn get_attribute<'a>(&'a self, name: &str, ns: Option<&str>) -> Option<&'a str> {
        self.attributes.get(&(name.to_string(), ns.map(|x| x.to_string()))).map(|x| &x[..])
    }

    /// Sets the attribute with the specified name and namespace.
    /// Returns the original value.
    pub fn set_attribute(&mut self, name: String, ns: Option<String>,
                         value: String) -> Option<String> {
        self.attributes.insert((name, ns), value)
    }

    /// Remove the attribute with the specified name and namespace.
    /// Returns the original value.
    pub fn remove_attribute(&mut self, name: &str, ns: Option<&str>) -> Option<String> {
        self.attributes.remove(&(name.to_string(), ns.map(|x| x.to_string())))
    }

    /// Gets the first child `Element` with the specified name and namespace. When no child
    /// with the specified name exists `None` is returned.
    pub fn get_child<'a>(&'a self, name: &str, ns: Option<&str>) -> Option<&'a Element> {
        for child in &self.children {
            if let Xml::ElementNode(ref elem) = *child {
                if elem.name != name {
                    continue;
                }
                match (ns, elem.ns.as_ref().map(|x| &x[..])) {
                    (Some(x), Some(y)) if x == y => return Some(elem),
                    (None, None) => return Some(elem),
                    _ => ()
                }
            }
        }
        None
    }

    /// Get all children `Element` with the specified name and namespace. When no child
    /// with the specified name exists an empty vetor is returned.
    pub fn get_children<'a>(&'a self, name: &str, ns: Option<&str>) -> Vec<&'a Element> {
        let mut res: Vec<&'a Element> = Vec::new();
        for child in &self.children {
            if let Xml::ElementNode(ref elem) = *child {
                if elem.name != name {
                    continue;
                }
                match (ns, elem.ns.as_ref().map(|x| &x[..])) {
                    (Some(x), Some(y)) if x == y => res.push(elem),
                    (None, None) => res.push(elem),
                    _ => ()
                }
            }
        }
        res
    }

    /// Appends a child element. Returns a reference to the added element.
    pub fn tag(&mut self, child: Element) -> &mut Element {
        self.children.push(Xml::ElementNode(child));
        let error = "Internal error: Could not get reference to new element!";
        let elem = match self.children.last_mut().expect(error) {
            &mut Xml::ElementNode(ref mut elem) => elem,
            _ => panic!(error)
        };
        elem
    }

    /// Appends a child element. Returns a mutable reference to self.
    pub fn tag_stay(&mut self, child: Element) -> &mut Element {
        self.children.push(Xml::ElementNode(child));
        self
    }

    /// Appends characters. Returns a mutable reference to self.
    pub fn text(&mut self, text: String) -> &mut Element {
        self.children.push(Xml::CharacterNode(text));
        self
    }

    /// Appends CDATA. Returns a mutable reference to self.
    pub fn cdata(&mut self, text: String) -> &mut Element {
        self.children.push(Xml::CDATANode(text));
        self
    }

    /// Appends a comment. Returns a mutable reference to self.
    pub fn comment(&mut self, text: String) -> &mut Element {
        self.children.push(Xml::CommentNode(text));
        self
    }

    /// Appends processing information. Returns a mutable reference to self.
    pub fn pi(&mut self, text: String) -> &mut Element {
        self.children.push(Xml::PINode(text));
        self
    }
}

impl FromStr for Element {
    type Err = BuilderError;
    #[inline]
    fn from_str(data: &str) -> Result<Element, BuilderError> {
        let mut p = Parser::new();
        let mut e = ElementBuilder::new();

        p.feed_str(data);
        for event in p.filter_map(|x| e.handle_event(x)) {
            return event;
        }
        Err(BuilderError::NoElement)
    }
}
