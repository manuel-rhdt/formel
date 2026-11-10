use std::io::BufRead;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::error::{ErrorType, ParsingError, Result};
use super::escape::StringExtUnescape;
use crate::host::MathElement;
use crate::standalone::{MathDocument, NodeId};

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
/// implicit `<math>` root. Entities are resolved with the HTML/MathML entity table.
pub fn parse<R: BufRead>(input: R) -> Result<MathDocument> {
    let mut reader = Reader::from_reader(input);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut document = MathDocument::new();
    let mut stack: Vec<OpenElement> = Vec::new();
    let mut top_level: Vec<NodeId> = Vec::new();

    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|error| ParsingError::at(reader.error_position(), ErrorType::XmlError(error)))?;
        match event {
            Event::Start(ref element) => {
                if let Some(token) = stack.last() {
                    if MathElement::from_local_name(&token.name).is_token() {
                        return Err(ParsingError::at(
                            reader.buffer_position(),
                            ErrorType::OtherError(format!(
                                "<{}> inside the token element <{}> is not supported",
                                local_name(element)?,
                                token.name
                            )),
                        ));
                    }
                }
                stack.push(open_element(element)?);
            }
            Event::Empty(ref element) => {
                let id = finish(&mut document, open_element(element)?);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(id),
                    None => top_level.push(id),
                }
            }
            Event::Text(ref text) => {
                if let Some(open) = stack.last_mut() {
                    open.text.push_str(text);
                }
            }
            Event::CData(ref data) => {
                if let Some(open) = stack.last_mut() {
                    open.text.push_str(data);
                }
            }
            // `&name;` and `&#x…;`, which quick-xml reports separately from the text
            Event::GeneralRef(ref reference) => {
                if let Some(open) = stack.last_mut() {
                    let escaped = format!("&{};", &**reference);
                    open.text.push_str(&escaped.unescape()?);
                }
            }
            Event::End(ref element) => {
                let name = element.local_name().into_inner().to_owned();
                let open = match stack.pop() {
                    Some(open) if open.name == name => open,
                    _ => {
                        return Err(ParsingError::at(
                            reader.buffer_position(),
                            ErrorType::WrongEndElement(name),
                        ));
                    }
                };
                let id = finish(&mut document, open);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(id),
                    None => top_level.push(id),
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }

    if !stack.is_empty() {
        return Err(ParsingError::at(
            reader.buffer_position(),
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

/// The element name without a namespace prefix.
fn local_name(element: &BytesStart) -> Result<String> {
    Ok(element.local_name().into_inner().to_owned())
}

fn open_element(element: &BytesStart) -> Result<OpenElement> {
    let mut attributes = Vec::new();
    for attribute in element.attributes() {
        let attribute = attribute.map_err(quick_xml::Error::from)?;
        let key = attribute.key.local_name().into_inner().to_owned();
        let value = attribute.value.unescape()?.into_owned();
        attributes.push((key, value));
    }
    Ok(OpenElement {
        name: local_name(element)?,
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
    } else if open.name == "mfenced" {
        expand_mfenced(document, open)
    } else {
        document.add_element(&open.name, open.attributes, open.children)
    }
}

/// Replaces the deprecated `<mfenced>` with the `<mrow>` it stands for: the `open` fence
/// (default `(`), the children separated by the `separators` (default `,`; the last one
/// repeats) and the `close` fence (default `)`).
fn expand_mfenced(document: &mut MathDocument, open: OpenElement) -> NodeId {
    let attribute = |name: &str| {
        open.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.trim().to_owned())
    };
    let fence = |document: &mut MathDocument, text: &str, role: &str| {
        let attributes = vec![(role.to_owned(), "true".to_owned())];
        document.add_token("mo", attributes, text)
    };
    let separators: Vec<char> = attribute("separators")
        .unwrap_or_else(|| ",".to_owned())
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    let mut children = Vec::new();
    let opening = attribute("open").unwrap_or_else(|| "(".to_owned());
    if !opening.is_empty() {
        children.push(fence(document, &opening, "fence"));
    }
    let count = open.children.len();
    for (i, child) in open.children.into_iter().enumerate() {
        children.push(child);
        if i + 1 < count {
            if let Some(&separator) = separators.get(i).or(separators.last()) {
                children.push(fence(document, &separator.to_string(), "separator"));
            }
        }
    }
    let closing = attribute("close").unwrap_or_else(|| ")".to_owned());
    if !closing.is_empty() {
        children.push(fence(document, &closing, "fence"));
    }
    document.add_element("mrow", Vec::new(), children)
}
