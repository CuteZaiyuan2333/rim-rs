use std::io::{Cursor, Write};

use quick_xml::Writer;
use quick_xml::events::{BytesCData, BytesDecl, BytesEnd, BytesStart, BytesText, Event};

use crate::error::{Error, Result};

/// A node in the XML tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    /// Escaped text (safe for user-provided strings).
    Text(String),
    /// Raw XML, inserted verbatim.
    Raw(String),
    Comment(String),
    CData(String),
}

impl From<Element> for Node {
    fn from(value: Element) -> Self {
        Self::Element(value)
    }
}

/// A small, ordered XML element tree.
///
/// Attribute order and child order are preserved. Text is escaped by
/// `quick-xml`; use [`Element::raw`] when verbatim XML is required.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.push((name.into(), value.into()));
        self
    }

    pub fn child(mut self, child: impl Into<Node>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.children.push(Node::Text(text.into()));
        self
    }

    pub fn raw(mut self, raw: impl Into<String>) -> Self {
        self.children.push(Node::Raw(raw.into()));
        self
    }

    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.children.push(Node::Comment(comment.into()));
        self
    }

    pub fn cdata(mut self, cdata: impl Into<String>) -> Self {
        self.children.push(Node::CData(cdata.into()));
        self
    }

    /// Appends `<name>value</name>`.
    pub fn child_text(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.children
            .push(Node::Element(Element::new(name).text(value)));
        self
    }

    /// Appends a child in-place. Useful inside `&mut self` closures.
    pub fn push_child(&mut self, child: impl Into<Node>) -> &mut Self {
        self.children.push(child.into());
        self
    }

    /// Appends `<name>value</name>` in-place.
    pub fn push_child_text(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> &mut Self {
        self.children
            .push(Node::Element(Element::new(name).text(value)));
        self
    }

    /// Finds the first child element with `name`, creating it if needed.
    pub fn ensure_child(&mut self, name: &str) -> &mut Element {
        let index = self
            .children
            .iter()
            .position(|node| matches!(node, Node::Element(el) if el.name == name));

        let index = match index {
            Some(index) => index,
            None => {
                self.children.push(Node::Element(Element::new(name)));
                self.children.len() - 1
            }
        };

        match &mut self.children[index] {
            Node::Element(el) => el,
            _ => unreachable!("just inserted an element"),
        }
    }

    /// Serializes this element (and its children) as a complete XML document.
    pub fn to_xml_string(&self) -> Result<String> {
        let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b'\t', 1);
        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
            .map_err(xml_error)?;
        self.write_element(&mut writer)?;

        let bytes = writer.into_inner().into_inner();
        String::from_utf8(bytes).map_err(|err| Error::Xml(err.to_string()))
    }

    fn write_element<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut start = BytesStart::new(self.name.as_str());
        for (name, value) in &self.attrs {
            start.push_attribute((name.as_str(), value.as_str()));
        }

        if self.children.is_empty() {
            writer.write_event(Event::Empty(start)).map_err(xml_error)?;
            return Ok(());
        }

        writer.write_event(Event::Start(start)).map_err(xml_error)?;

        for child in &self.children {
            match child {
                Node::Element(element) => element.write_element(writer)?,
                Node::Text(text) => {
                    writer
                        .write_event(Event::Text(BytesText::new(text)))
                        .map_err(xml_error)?;
                }
                Node::Raw(raw) => {
                    // `quick-xml` has no raw event. Writing directly to the
                    // underlying writer is the cleanest escape hatch.
                    writer
                        .get_mut()
                        .write_all(raw.as_bytes())
                        .map_err(|err| Error::Xml(err.to_string()))?;
                }
                Node::Comment(comment) => {
                    writer
                        .write_event(Event::Comment(BytesText::new(comment)))
                        .map_err(xml_error)?;
                }
                Node::CData(cdata) => {
                    // Split on `]]>` so the generated document stays well-formed.
                    for part in BytesCData::escaped(cdata) {
                        writer.write_event(Event::CData(part)).map_err(xml_error)?;
                    }
                }
            }
        }

        writer
            .write_event(Event::End(BytesEnd::new(self.name.as_str())))
            .map_err(xml_error)?;
        Ok(())
    }
}

fn xml_error(err: std::io::Error) -> Error {
    Error::Xml(err.to_string())
}

/// Small constructors for common XML shapes.
pub struct Xml;

impl Xml {
    pub fn element(name: impl Into<String>) -> Element {
        Element::new(name)
    }

    pub fn text(name: impl Into<String>, value: impl Into<String>) -> Element {
        Element::new(name).text(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_text_and_attributes() {
        let element = Element::new("description")
            .attr("data", "a&b")
            .text("Use <b>bold</b> & don't \"break\"");
        let xml = element.to_xml_string().unwrap();

        assert!(xml.contains("&lt;b&gt;bold&lt;/b&gt;"));
        assert!(xml.contains("&amp;"));
        assert!(xml.contains("&apos;"));
        assert!(xml.contains("data=\"a&amp;b\""));
        assert!(!xml.contains("<b>"));
    }

    #[test]
    fn raw_fragment_is_not_escaped() {
        let element =
            Element::new("comps").child(Element::new("li").raw("<glowRadius>4</glowRadius>"));
        let xml = element.to_xml_string().unwrap();
        assert!(xml.contains("<glowRadius>4</glowRadius>"));
    }

    #[test]
    fn preserves_attribute_order() {
        let element = Element::new("ThingDef")
            .attr("ParentName", "BaseThing")
            .attr("Name", "MyBase");
        let xml = element.to_xml_string().unwrap();
        let parent = xml.find("ParentName").unwrap();
        let name = xml.find("Name").unwrap();
        assert!(parent < name);
    }

    #[test]
    fn empty_element_is_self_closing() {
        let xml = Element::new("Defs").to_xml_string().unwrap();
        assert!(xml.contains("<Defs/>"));
    }

    #[test]
    fn supports_comments_and_cdata() {
        let element = Element::new("root").comment("hello").cdata("<not-escaped>");
        let xml = element.to_xml_string().unwrap();
        assert!(xml.contains("<!--hello-->"));
        assert!(xml.contains("<![CDATA[<not-escaped>]]>"));
    }

    #[test]
    fn splits_invalid_cdata_terminators() {
        let element = Element::new("root").cdata("a]]>b");
        let xml = element.to_xml_string().unwrap();
        assert!(xml.contains("<![CDATA[a]]]]><![CDATA[>b]]>"));
    }
}
