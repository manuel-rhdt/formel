//! A built-in version of the MathML Core user agent stylesheet, plus the presentational
//! attributes `displaystyle`, `scriptlevel`, `mathsize` and `mathvariant`.

use super::{MathDocument, NodeId, StyleOptions};
use crate::host::{
    Edges, MathComputedStyle, MathElement, MathFont, MathShift, MathStyle, parse_math_length,
    script_scale_factor,
};
use crate::unicode_math::{Family, StringExtMathml};

pub(super) fn resolve<F: MathFont>(
    document: &MathDocument,
    font: &F,
    options: StyleOptions,
) -> (Vec<MathComputedStyle>, Vec<Option<String>>) {
    let root_style = MathComputedStyle {
        font_size: options.font_size,
        math_style: options.math_style,
        ..MathComputedStyle::default()
    };
    let mut resolver = Resolver {
        document,
        font,
        styles: vec![root_style; document.len()],
        texts: vec![None; document.len()],
    };
    if let Some(root) = document.root() {
        resolver.resolve(root, None, &root_style);
    }
    (resolver.styles, resolver.texts)
}

struct Resolver<'a, F> {
    document: &'a MathDocument,
    font: &'a F,
    styles: Vec<MathComputedStyle>,
    texts: Vec<Option<String>>,
}

impl<'a, F: MathFont> Resolver<'a, F> {
    fn resolve(
        &mut self,
        node: NodeId,
        parent: Option<(NodeId, usize)>,
        parent_style: &MathComputedStyle,
    ) {
        let doc = self.document;
        let element = doc.node(node).element;
        // inherited properties
        let mut style = MathComputedStyle {
            font_size: parent_style.font_size,
            math_depth: parent_style.math_depth,
            math_style: parent_style.math_style,
            math_shift: parent_style.math_shift,
            direction: parent_style.direction,
            ..MathComputedStyle::default()
        };
        if let Some((parent, index)) = parent {
            self.apply_ua_rules(&mut style, parent, index, parent_style);
        }

        // presentational attributes
        if element == MathElement::Math {
            match doc.attribute(node, "display").map(str::trim) {
                Some("block") => style.math_style = MathStyle::Normal,
                Some("inline") => style.math_style = MathStyle::Compact,
                _ => {}
            }
        }
        // Table content is laid out in inline style (MathML Core: `mtable { math-style:
        // compact }`); a `displaystyle` attribute on the table still wins.
        if element == MathElement::Mtable {
            style.math_style = MathStyle::Compact;
        }
        match doc.attribute(node, "displaystyle").map(str::trim) {
            Some("true") => style.math_style = MathStyle::Normal,
            Some("false") => style.math_style = MathStyle::Compact,
            _ => {}
        }
        if let Some(level) = doc.attribute(node, "scriptlevel").map(str::trim) {
            if let Some(delta) = level.strip_prefix('+') {
                style.math_depth += delta.parse::<i32>().unwrap_or(0);
            } else if level.starts_with('-') {
                style.math_depth += level.parse::<i32>().unwrap_or(0);
            } else if let Ok(absolute) = level.parse::<i32>() {
                style.math_depth = absolute;
            }
        }

        // font-size: math
        style.font_size *=
            script_scale_factor(self.font, parent_style.math_depth, style.math_depth);
        if let Some(size) = doc.attribute(node, "mathsize").map(str::trim) {
            if let Some(percent) = size.strip_suffix('%') {
                if let Ok(percent) = percent.trim().parse::<f32>() {
                    style.font_size *= percent / 100.0;
                }
            } else if let Some(px) = parse_math_length(size, &style) {
                style.font_size = px;
            }
        }

        // MathML Core: `mtd { padding: 0.5ex 0.4em }`. Without access to the font's x-height,
        // 1ex is approximated as 0.5em, as in `parse_math_length`.
        if element == MathElement::Mtd {
            style.padding = Edges {
                top: 0.25 * style.font_size,
                right: 0.4 * style.font_size,
                bottom: 0.25 * style.font_size,
                left: 0.4 * style.font_size,
            };
            // Inline `style="padding-left: 0; ..."`, as produced by LaTeX-to-MathML converters.
            if let Some(inline) = doc.attribute(node, "style") {
                apply_inline_padding(&mut style, inline);
            }
        }

        if element.is_token() {
            self.texts[node] = doc.node(node).text.as_deref().map(|text| {
                let variant = doc.attribute(node, "mathvariant").and_then(Family::from_mathvariant);
                let text = match (element, variant) {
                    // text-transform: math-auto
                    (MathElement::Mi, None) => text.adapt_to_family(None),
                    (_, None) => text.into(),
                    (_, Some(family)) => text.adapt_to_family(Some(family)),
                };
                text.replace_anomalous_characters(element == MathElement::Mo)
            });
        }

        self.styles[node] = style;
        for (index, &child) in doc.node(node).children.iter().enumerate() {
            self.resolve(child, Some((node, index)), &style);
        }
    }

    /// The rules of the MathML Core user agent stylesheet that depend on the parent.
    fn apply_ua_rules(
        &self,
        style: &mut MathComputedStyle,
        parent: NodeId,
        index: usize,
        parent_style: &MathComputedStyle,
    ) {
        use MathElement::*;
        let doc = self.document;
        match doc.node(parent).element {
            Mfrac => {
                // math-depth: auto-add
                if parent_style.math_style == MathStyle::Compact {
                    style.math_depth += 1;
                }
                style.math_style = MathStyle::Compact;
                if index == 1 {
                    style.math_shift = MathShift::Compact;
                }
            }
            Msub | Msup | Msubsup | Mmultiscripts if index > 0 => {
                style.math_depth += 1;
                style.math_style = MathStyle::Compact;
                if self.is_subscript(parent, index) {
                    style.math_shift = MathShift::Compact;
                }
            }
            Munder | Mover | Munderover if index > 0 => {
                style.math_style = MathStyle::Compact;
                // Accents are not scripts: they keep the base's math-depth.
                if !self.script_is_accent(parent, index) {
                    style.math_depth += 1;
                }
            }
            Munder | Mover | Munderover => {
                // An accented base is cramped.
                let over = match doc.node(parent).element {
                    Mover => Some(1),
                    Munderover => Some(2),
                    _ => None,
                };
                if over.is_some_and(|over| self.script_is_accent(parent, over)) {
                    style.math_shift = MathShift::Compact;
                }
            }
            Mroot if index > 0 => {
                style.math_depth += 2;
                style.math_style = MathStyle::Compact;
            }
            Msqrt | Mroot => style.math_shift = MathShift::Compact,
            _ => {}
        }
    }

    fn is_subscript(&self, parent: NodeId, index: usize) -> bool {
        let node = self.document.node(parent);
        match node.element {
            MathElement::Msub | MathElement::Msubsup => index == 1,
            MathElement::Mmultiscripts => {
                // Scripts come in (sub, sup) pairs, before and after <mprescripts/>.
                let prescripts = node.children.iter().position(|&c| {
                    self.document.node(c).element == MathElement::Mprescripts
                });
                match prescripts {
                    Some(p) if index > p => (index - p) % 2 == 1,
                    _ => index % 2 == 1,
                }
            }
            _ => false,
        }
    }

    /// Whether the script at `index` of an under/over element is an accent. As in MathML
    /// Core, only the `accent`/`accentunder` attribute decides.
    fn script_is_accent(&self, parent: NodeId, index: usize) -> bool {
        let attribute = match (self.document.node(parent).element, index) {
            (MathElement::Mover, 1) | (MathElement::Munderover, 2) => "accent",
            (MathElement::Munder, 1) | (MathElement::Munderover, 1) => "accentunder",
            _ => return false,
        };
        self.document
            .attribute(parent, attribute)
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
    }
}

/// The value of `property` in an inline `style` attribute (the last declaration wins).
pub(super) fn inline_style_property<'a>(style_attribute: &'a str, property: &str) -> Option<&'a str> {
    style_attribute
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(name, _)| name.trim().eq_ignore_ascii_case(property))
        .map(|(_, value)| value.trim())
        .last()
}

/// Applies the `padding` shorthand and the `padding-*` longhands of an inline style attribute.
fn apply_inline_padding(style: &mut MathComputedStyle, inline: &str) {
    let length = |value: &str| {
        // A unitless zero is a valid CSS length.
        if value.trim() == "0" {
            Some(0.0)
        } else {
            parse_math_length(value, style)
        }
    };
    let mut padding = style.padding;
    // declarations apply in order, so a later longhand overrides an earlier shorthand
    for declaration in inline.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "padding" => {
                let values: Vec<f32> = value.split_whitespace().filter_map(length).collect();
                padding = match values.as_slice() {
                    [all] => Edges { top: *all, right: *all, bottom: *all, left: *all },
                    [v, h] => Edges { top: *v, right: *h, bottom: *v, left: *h },
                    [t, h, b] => Edges { top: *t, right: *h, bottom: *b, left: *h },
                    [t, r, b, l] => Edges { top: *t, right: *r, bottom: *b, left: *l },
                    _ => padding,
                };
            }
            "padding-top" => padding.top = length(value).unwrap_or(padding.top),
            "padding-right" => padding.right = length(value).unwrap_or(padding.right),
            "padding-bottom" => padding.bottom = length(value).unwrap_or(padding.bottom),
            "padding-left" => padding.left = length(value).unwrap_or(padding.left),
            _ => {}
        }
    }
    style.padding = padding;
}
