use std::io::BufRead;

use super::error::{ErrorType, ParsingError, Result};
use super::escape::StringExtUnescape;
use crate::host::MathElement;
use crate::standalone::{MathDocument, NodeId};

pub use quick_xml::{Element, Event, XmlReader};

/// An element whose end tag hasn't been read yet.
struct OpenElement {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<NodeId>,
    text: String,
}

/// Parses MathML markup into a [`MathDocument`].
///
/// If the input isn't a single `<math>` element, its top-level elements are wrapped into an
/// implicit `<math>` root.
pub fn parse<R: BufRead>(input: R) -> Result<MathDocument> {
    let mut parser = XmlReader::from_reader(input).trim_text(true);
    let mut document = MathDocument::new();
    let mut stack: Vec<OpenElement> = Vec::new();
    let mut top_level: Vec<NodeId> = Vec::new();

    while let Some(event) = parser.next() {
        match event? {
            Event::Start(ref element) => {
                if let Some(token) = stack.last() {
                    if MathElement::from_local_name(&token.name).is_token() {
                        return Err(ParsingError::from_string(
                            &parser,
                            format!(
                                "<{}> inside the token element <{}> is not supported",
                                String::from_utf8_lossy(element.name()),
                                token.name
                            ),
                        ));
                    }
                }
                stack.push(open_element(element)?);
            }
            Event::Empty(ref element) => {
                let open = open_element(element)?;
                let id = finish(&mut document, open);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(id),
                    None => top_level.push(id),
                }
            }
            Event::Text(ref text) => {
                if let Some(open) = stack.last_mut() {
                    let text = std::str::from_utf8(text.content())?.unescape()?;
                    open.text.push_str(&text);
                }
            }
            Event::End(ref element) => {
                let name = local_name(element.name())?;
                let open = match stack.pop() {
                    Some(open) if open.name == name => open,
                    _ => {
                        return Err(ParsingError::of_type(
                            &parser,
                            ErrorType::WrongEndElement(name.to_owned()),
                        ));
                    }
                };
                let id = finish(&mut document, open);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(id),
                    None => top_level.push(id),
                }
            }
            _ => {}
        }
    }

    if !stack.is_empty() {
        return Err(ParsingError::of_type(
            &parser,
            ErrorType::UnexpectedEndOfInput,
        ));
    }
    let root = match top_level.as_slice() {
        [single] if document.node(*single).element == MathElement::Math => *single,
        _ => document.add_element("math", Vec::new(), top_level),
    };
    document.set_root(root);
    Ok(document)
}

/// The name without a namespace prefix.
fn local_name(name: &[u8]) -> Result<&str> {
    let name = std::str::from_utf8(name)?;
    Ok(name.rsplit(':').next().unwrap_or(name))
}

fn open_element(element: &Element) -> Result<OpenElement> {
    let mut attributes = Vec::new();
    for attribute in element.attributes() {
        let (key, value) = attribute?;
        let key = local_name(key)?.to_owned();
        let value = std::str::from_utf8(value)?.unescape()?.into_owned();
        attributes.push((key, value));
    }
    Ok(OpenElement {
        name: local_name(element.name())?.to_owned(),
        attributes,
        children: Vec::new(),
        text: String::new(),
    })
}

fn finish(document: &mut MathDocument, open: OpenElement) -> NodeId {
    if MathElement::from_local_name(&open.name).is_token() {
        // Only XML whitespace is trimmed; e.g. a no-break space is content.
        let text = open
            .text
            .trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'));
        document.add_token(&open.name, open.attributes, text)
    } else {
        document.add_element(&open.name, open.attributes, open.children)
    }
}
