//! `<mpadded>` and `<menclose>`.

use super::lbox::LBox;
use super::{Box_, Ctx, Engine};
use crate::host::{
    Form, MathComputedStyle, MathConstant, MathLayoutHost, Point, parse_math_length,
};

/// The size of an `<mpadded>`'s content, which relative values refer to.
#[derive(Clone, Copy)]
struct Dimensions {
    width: f32,
    height: f32,
    depth: f32,
}

/// A parsed `<mpadded>` attribute value.
struct PaddedLength {
    value: f32,
    /// The value had a `+` or `-` sign: it changes the content's dimension instead of
    /// replacing it.
    relative: bool,
}

/// Parses an `<mpadded>` value: a length, a percentage or a multiple of `width`, `height` or
/// `depth` (relative to that dimension of the content), optionally signed. `reference` is the
/// dimension the percentage refers to.
fn parse_padded_length(
    value: &str,
    reference: f32,
    dims: Dimensions,
    style: &MathComputedStyle,
) -> Option<PaddedLength> {
    let value = value.trim();
    let (sign, unsigned) = match value.as_bytes().first()? {
        b'+' => (1.0, &value[1..]),
        b'-' => (-1.0, &value[1..]),
        _ => (1.0, value),
    };
    let relative = unsigned.len() != value.len();
    let split = unsigned
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(unsigned.len());
    let (number, unit) = unsigned.split_at(split);
    let magnitude = match unit.trim() {
        "%" => number.parse::<f32>().ok()? / 100.0 * reference,
        "width" => number.parse::<f32>().ok()? * dims.width,
        "height" => number.parse::<f32>().ok()? * dims.height,
        "depth" => number.parse::<f32>().ok()? * dims.depth,
        _ => parse_math_length(unsigned, style)?,
    };
    Some(PaddedLength {
        value: sign * magnitude,
        relative,
    })
}

/// The `notation` values of `<menclose>` that mlayout draws.
#[derive(Default)]
struct Notation {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
    up_strike: bool,
    down_strike: bool,
    horizontal_strike: bool,
    vertical_strike: bool,
    radical: bool,
}

impl Notation {
    fn parse(value: &str) -> Notation {
        let mut n = Notation::default();
        for word in value.split_whitespace() {
            match word {
                // A rounded box is drawn with square corners.
                "box" | "roundedbox" => {
                    n.left = true;
                    n.right = true;
                    n.top = true;
                    n.bottom = true;
                }
                "left" => n.left = true,
                "right" => n.right = true,
                "top" => n.top = true,
                "bottom" => n.bottom = true,
                // Long division: the curved line is drawn straight.
                "longdiv" => {
                    n.left = true;
                    n.top = true;
                }
                "actuarial" => {
                    n.right = true;
                    n.top = true;
                }
                "radical" => n.radical = true,
                "updiagonalstrike" => n.up_strike = true,
                "downdiagonalstrike" => n.down_strike = true,
                "horizontalstrike" => n.horizontal_strike = true,
                "verticalstrike" => n.vertical_strike = true,
                _ => {}
            }
        }
        n
    }
}

impl<'t, T: MathLayoutHost> Engine<'t, T> {
    /// `<mpadded>`: the content with its width, height, depth and position adjusted.
    pub(crate) fn layout_padded(
        &mut self,
        node: T::NodeId,
        children: &[T::NodeId],
        style: &MathComputedStyle,
        ctx: Ctx,
    ) -> Box_<T> {
        let mut content = self.layout_row(node, children, ctx);
        let dims = Dimensions {
            width: content.metrics.advance,
            height: content.metrics.ascent,
            depth: content.metrics.descent,
        };
        let attribute = |engine: &Self, name: &str, reference: f32| {
            engine
                .tree
                .attribute(node, name)
                .and_then(|value| parse_padded_length(&value, reference, dims, style))
        };
        let resolve = |parsed: Option<PaddedLength>, original: f32| match parsed {
            Some(p) if p.relative => original + p.value,
            Some(p) => p.value,
            None => original,
        };

        let lspace = attribute(self, "lspace", dims.width).map_or(0.0, |p| p.value);
        let voffset = attribute(self, "voffset", dims.height).map_or(0.0, |p| p.value);
        let width_attribute = attribute(self, "width", dims.width);
        let height = resolve(attribute(self, "height", dims.height), dims.height).max(0.0);
        let depth = resolve(attribute(self, "depth", dims.depth), dims.depth).max(0.0);
        // Without a `width`, the box grows by `lspace` so that the content isn't pushed out of it.
        let width = match width_attribute {
            Some(p) if p.relative => dims.width + p.value,
            Some(p) => p.value,
            None => dims.width + lspace,
        }
        .max(0.0);

        content.origin.x += lspace;
        content.origin.y -= voffset;
        let mut padded = LBox::boxes(vec![content]);
        padded.metrics.advance = width;
        padded.metrics.ascent = height;
        padded.metrics.descent = depth;
        padded
    }

    /// `<menclose>`: the content with the lines of its `notation` drawn around it (default
    /// `longdiv`).
    pub(crate) fn layout_enclose(
        &mut self,
        node: T::NodeId,
        children: &[T::NodeId],
        style: &MathComputedStyle,
        _ctx: Ctx,
    ) -> Box_<T> {
        let notation = Notation::parse(
            &self
                .tree
                .attribute(node, "notation")
                .map_or_else(|| "longdiv".to_owned(), |v| v.into_owned()),
        );
        let mut content = if notation.radical {
            self.layout_radical(node, None, children, style)
        } else {
            self.layout_row(node, children, Ctx::plain(Form::Infix))
        };
        let font = self.font(node, style);
        let thickness = font.constant(MathConstant::FractionRuleThickness);
        let axis = font.constant(MathConstant::AxisHeight);
        let padding = style.font_size * 0.2;
        let side = |present: bool| if present { padding + thickness } else { 0.0 };
        let (left, right) = (side(notation.left), side(notation.right));
        let (top, bottom) = (side(notation.top), side(notation.bottom));

        let inner = content.metrics;
        let width = left + inner.advance + right;
        let ascent = top + inner.ascent;
        let descent = bottom + inner.descent;
        let height = ascent + descent;
        content.origin.x += left;

        let mut parts = vec![content];
        // A bar of the given size whose bottom left corner is at (x, y).
        let mut bar = |x: f32, y: f32, w: f32, h: f32| {
            let mut rect = LBox::rect(w, h, false);
            rect.origin = Point::new(x, y);
            parts.push(rect);
        };
        if notation.left {
            bar(0.0, descent, thickness, height);
        }
        if notation.right {
            bar(width - thickness, descent, thickness, height);
        }
        if notation.top {
            bar(0.0, -ascent + thickness, width, thickness);
        }
        if notation.bottom {
            bar(0.0, descent, width, thickness);
        }
        if notation.horizontal_strike {
            bar(0.0, -axis + thickness / 2.0, width, thickness);
        }
        if notation.vertical_strike {
            bar(
                (width - thickness) / 2.0,
                descent,
                thickness,
                height,
            );
        }
        let mut line = |from: Point, dx: f32, dy: f32| {
            let mut line = LBox::line(dx, dy, thickness);
            line.origin = from;
            parts.push(line);
        };
        if notation.up_strike {
            line(Point::new(0.0, descent), width, -height);
        }
        if notation.down_strike {
            line(Point::new(0.0, -ascent), width, height);
        }

        let attachment = parts[0].origin.x + parts[0].metrics.top_accent_attachment;
        let mut enclosure = LBox::boxes(parts);
        enclosure.metrics.advance = width;
        enclosure.metrics.ascent = ascent;
        enclosure.metrics.descent = descent;
        enclosure.metrics.ink_left = 0.0;
        enclosure.metrics.ink_width = width;
        enclosure.metrics.italic_correction = 0.0;
        enclosure.metrics.top_accent_attachment = attachment;
        enclosure
    }
}
