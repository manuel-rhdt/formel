//! MathML parsing and the operator dictionary.
//!
//! With the `mathml_parser` feature, [`parse`] reads MathML markup into a
//! [`MathDocument`](crate::standalone::MathDocument).

mod error;
#[cfg(feature = "mathml_parser")]
mod escape;
mod operator;
pub(crate) mod operator_dict;
#[cfg(feature = "mathml_parser")]
mod xml_reader;

pub use self::error::{ErrorType, ParsingError, Result};
pub use self::operator::{Flags, Form};
#[cfg(feature = "mathml_parser")]
pub use self::xml_reader::parse;
pub use crate::unicode_math::StringExtMathml;

#[cfg(all(test, feature = "mathml_parser"))]
mod tests {
    use super::*;
    use crate::host::MathElement;

    #[test]
    fn parse_wraps_fragments_in_math() {
        let doc = parse("<mi>x</mi><mo>+</mo><mn>1</mn>".as_bytes()).unwrap();
        let root = doc.root().unwrap();
        assert_eq!(doc.node(root).element, MathElement::Math);
        assert_eq!(doc.node(root).children.len(), 3);
    }

    #[test]
    fn parse_keeps_math_root_and_attributes() {
        let xml = r#"<math display="block"><mfrac linethickness="0"><mi>a</mi><mspace width="1em"/></mfrac></math>"#;
        let doc = parse(xml.as_bytes()).unwrap();
        let root = doc.root().unwrap();
        assert_eq!(doc.attribute(root, "display"), Some("block"));
        let frac = doc.node(root).children[0];
        assert_eq!(doc.node(frac).element, MathElement::Mfrac);
        assert_eq!(doc.attribute(frac, "linethickness"), Some("0"));
        let space = doc.node(frac).children[1];
        assert_eq!(doc.node(space).element, MathElement::Mspace);
    }

    #[test]
    fn parse_unescapes_token_text() {
        let doc = parse("<mo>&#x2212;</mo>".as_bytes()).unwrap();
        let root = doc.root().unwrap();
        let mo = doc.node(root).children[0];
        assert_eq!(doc.node(mo).text.as_deref(), Some("\u{2212}"));
    }

    #[test]
    fn parse_rejects_mismatched_tags() {
        assert!(parse("<mrow><mi>x</mi></mfrac>".as_bytes()).is_err());
        assert!(parse("<mrow><mi>x</mi>".as_bytes()).is_err());
    }
}
