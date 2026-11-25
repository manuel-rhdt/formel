use std::collections::HashMap;
use std::path::Path as FsPath;

use crate::host::{Fragment, NodeLayout, Point, Rect};
use crate::standalone::{MathDocument, NodeId};

use ::svg::node::element::{Group, Line, Path, Rectangle, Title};
use ::svg::node::Node;
use ::svg::Document;

use std::fmt::Write;

use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};
use rustybuzz::Face;

/// The space kept around a formula by default, as a fraction of the font size.
///
/// The margin is not a constant in the layout's unit: the engine's px are whatever `font-size`
/// the caller asked for, so a fixed 10 px is 0.01 em for a caller that lays out with
/// `font_size = units_per_em` and 0.63 em for one that keeps the default of 16 px.
pub const DEFAULT_MARGIN_EM: f32 = 0.05;

#[derive(Debug, Default, Clone, Copy)]
pub struct Flags {
    /// Fill the boxes of token elements.
    pub show_ink_bounds: bool,
    /// Outline the border box of every element.
    pub show_logical_bounds: bool,
    /// Space kept around the formula on every side, in the layout's unit (px) — not necessarily
    /// the font's. [`DEFAULT_MARGIN_EM`] times the font size is the usual choice; 0 keeps the box
    /// to the ink.
    pub margin: f32,
    /// The root's `font-size`, in the layout's unit. When set, the SVG gets `width` and `height`
    /// in `em`, so it takes its size from the surrounding text while the `viewBox` keeps the
    /// layout's own units.
    pub font_size: Option<f32>,
}

/// The `viewBox` of the rendered SVG: the root's border box, widened where the formula's ink
/// reaches past it — the hook of an integral, the overhang of an italic `f` — plus `margin` on
/// every side.
pub fn view_box(root: &NodeLayout, margin: f32) -> Rect {
    let left = root.ink_left.min(0.0);
    let right = (root.ink_left + root.ink_width).max(root.size.width);
    Rect::new(
        left - margin,
        -margin,
        right - left + 2.0 * margin,
        root.size.height + 2.0 * margin,
    )
}

/// What makes the SVG self-describing.
///
/// The title becomes the SVG's `<title>`, which is the accessible name an assistive technology
/// reads for the image, and turns on `role="img"`. No `id` and no `aria-labelledby` are written:
/// several SVGs are often inlined into one document, and duplicated ids would break that.
///
/// The default ([`A11y::SPOKEN`]) names every SVG: with the `alttext` of `<math>` if the author
/// gave one, and otherwise with the formula read aloud by [`speech`](crate::speech)
/// ("the square root of b squared minus 4 a c").
#[derive(Debug, Default, Clone, Copy)]
pub struct A11y<'a> {
    /// `None`: the author's `alttext`, else the spoken formula. `Some(title)`: exactly that
    /// title; `Some("")` writes no title and no role.
    pub title: Option<&'a str>,
}

impl A11y<'static> {
    /// Name the SVG after the author's `alttext` or the spoken formula (the default).
    pub const SPOKEN: A11y<'static> = A11y { title: None };
    /// Write no title and no role.
    pub const NONE: A11y<'static> = A11y { title: Some("") };
}

impl<'a> A11y<'a> {
    /// Name the SVG `title`.
    pub fn titled(title: &'a str) -> Self {
        A11y { title: Some(title) }
    }
}

/// Renders a laid-out document to an SVG file. See [`render_to_string`].
pub fn render<T: AsRef<FsPath>>(
    document: &MathDocument,
    layouts: &HashMap<NodeId, NodeLayout>,
    font: &Face<'_>,
    units_per_em: f32,
    flags: Flags,
    out_path: T,
) {
    render_with(
        document,
        layouts,
        font,
        units_per_em,
        flags,
        A11y::default(),
        out_path,
    )
}

/// [`render`], with an accessible name written into the SVG.
pub fn render_with<T: AsRef<FsPath>>(
    document: &MathDocument,
    layouts: &HashMap<NodeId, NodeLayout>,
    font: &Face<'_>,
    units_per_em: f32,
    flags: Flags,
    a11y: A11y<'_>,
    out_path: T,
) {
    let svg_document = build_document(document, layouts, font, units_per_em, flags, a11y);
    ::svg::save(out_path, &svg_document).unwrap();
}

/// Renders a laid-out document to an SVG string. Coordinates are px; `units_per_em` converts
/// glyph outlines from font units. See [`Flags::margin`] for the space around the formula and
/// [`Flags::font_size`] for the `em` size of the result.
pub fn render_to_string(
    document: &MathDocument,
    layouts: &HashMap<NodeId, NodeLayout>,
    font: &Face<'_>,
    units_per_em: f32,
    flags: Flags,
) -> String {
    render_to_string_with(
        document,
        layouts,
        font,
        units_per_em,
        flags,
        A11y::default(),
    )
}

/// [`render_to_string`], with an accessible name written into the SVG.
pub fn render_to_string_with(
    document: &MathDocument,
    layouts: &HashMap<NodeId, NodeLayout>,
    font: &Face<'_>,
    units_per_em: f32,
    flags: Flags,
    a11y: A11y<'_>,
) -> String {
    build_document(document, layouts, font, units_per_em, flags, a11y).to_string()
}

fn build_document(
    document: &MathDocument,
    layouts: &HashMap<NodeId, NodeLayout>,
    font: &Face<'_>,
    units_per_em: f32,
    flags: Flags,
    a11y: A11y<'_>,
) -> Document {
    let root = document.root().expect("document has a root");
    let root_layout = &layouts[&root];

    let mut svg_document = Document::new();
    let view = view_box(root_layout, flags.margin);
    svg_document.assign(
        "viewBox",
        (
            view.origin.x,
            view.origin.y,
            view.size.width,
            view.size.height,
        ),
    );
    // `em` rather than px: the formula then follows the font size of the text around it, and the
    // viewBox keeps the layout's own units for everything that draws.
    if let Some(font_size) = flags.font_size.filter(|size| *size > 0.0) {
        let em = |length: f32| format!("{:.3}em", length / font_size);
        svg_document.assign("width", em(view.size.width));
        svg_document.assign("height", em(view.size.height));
    }

    // The title has to be the first child, so it goes in before anything is drawn.
    let title = match a11y.title {
        Some(title) => title.trim().to_owned(),
        None => document
            .attribute(root, "alttext")
            .map(|alt| alt.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|alt| !alt.is_empty())
            .unwrap_or_else(|| crate::speech::speak_document(document, root)),
    };
    if !title.is_empty() {
        svg_document.assign("role", "img");
        svg_document.append(Title::new(title));
    }

    let mut ink_group = Group::new().set("stroke", "none").set("fill", "#FFE6E6");
    let mut logical_group = Group::new()
        .set("stroke", "#FF0000")
        .set("stroke-width", 2)
        .set("fill", "none");
    let mut black_group = Group::new().set("fill", "black").set("stroke", "none");

    let mut stack = vec![(root, Point::ZERO)];
    while let Some((node, parent_origin)) = stack.pop() {
        let Some(layout) = layouts.get(&node) else {
            continue;
        };
        let origin = parent_origin + layout.location;
        let rect = |fill_or_stroke: Rectangle| {
            fill_or_stroke
                .set("x", origin.x)
                .set("y", origin.y)
                .set("width", layout.size.width)
                .set("height", layout.size.height)
        };
        if document.node(node).element.is_token() {
            ink_group.append(rect(Rectangle::new()));
        }
        logical_group.append(rect(Rectangle::new()));

        for fragment in &layout.fragments {
            match fragment {
                Fragment::Glyphs {
                    origin: run_origin,
                    font_size,
                    glyphs,
                } => {
                    let scale = font_size / units_per_em;
                    for glyph in glyphs {
                        let x = origin.x + run_origin.x + glyph.offset.x;
                        let y = origin.y + run_origin.y + glyph.offset.y;
                        black_group.append(draw_glyph(font, glyph.glyph, x, y, scale));
                    }
                }
                Fragment::Rule(r) => black_group.append(
                    Rectangle::new()
                        .set("x", origin.x + r.origin.x)
                        .set("y", origin.y + r.origin.y)
                        .set("width", r.size.width)
                        .set("height", r.size.height),
                ),
                Fragment::Line {
                    start,
                    end,
                    thickness,
                } => black_group.append(
                    Line::new()
                        .set("x1", origin.x + start.x)
                        .set("y1", origin.y + start.y)
                        .set("x2", origin.x + end.x)
                        .set("y2", origin.y + end.y)
                        .set("stroke", "black")
                        .set("stroke-width", *thickness),
                ),
                Fragment::DashedRule(r) => {
                    // stroke along the longer side, through the middle of the rectangle
                    let thickness = r.size.width.min(r.size.height);
                    let (x1, y1, x2, y2) = if r.size.width >= r.size.height {
                        let y = origin.y + r.origin.y + r.size.height / 2.0;
                        (origin.x + r.origin.x, y, origin.x + r.right(), y)
                    } else {
                        let x = origin.x + r.origin.x + r.size.width / 2.0;
                        (x, origin.y + r.origin.y, x, origin.y + r.bottom())
                    };
                    black_group.append(
                        Line::new()
                            .set("x1", x1)
                            .set("y1", y1)
                            .set("x2", x2)
                            .set("y2", y2)
                            .set("stroke", "black")
                            .set("stroke-width", thickness)
                            .set("stroke-dasharray", format!("{} {}", 3.0 * thickness, 2.0 * thickness)),
                    )
                }
            }
        }

        for &child in &document.node(node).children {
            stack.push((child, origin));
        }
    }

    if flags.show_ink_bounds {
        svg_document.append(ink_group);
    }
    if flags.show_logical_bounds {
        svg_document.append(logical_group);
    }
    svg_document.append(black_group);
    svg_document
}

/// Collects a glyph outline as SVG path data.
#[derive(Debug, Default)]
struct PathBuilder {
    data: String,
}

impl PathBuilder {
    fn push(&mut self, command: std::fmt::Arguments) {
        let _ = write!(self.data, "{} ", command);
    }
}

impl OutlineBuilder for PathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.push(format_args!("M{} {}", x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.push(format_args!("L{} {}", x, y));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.push(format_args!("Q{} {} {} {}", cx, cy, x, y));
    }
    fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.push(format_args!("C{} {} {} {} {} {}", c1x, c1y, c2x, c2y, x, y));
    }
    fn close(&mut self) {
        self.push(format_args!("Z"));
    }
}

/// Draws a glyph from the font. Outlines are in font units with y pointing up, so
/// they are flipped and scaled to px.
fn draw_glyph(font: &Face<'_>, glyph: u32, x: f32, y: f32, scale: f32) -> Group {
    let mut builder = PathBuilder::default();
    font.outline_glyph(GlyphId(glyph as u16), &mut builder);
    let data = builder.data;
    Group::new()
        .set(
            "transform",
            format!("translate({:?}, {:?}) scale({:?}, {:?})", x, y, scale, -scale),
        )
        .add(Path::new().set("d", data.trim_end().to_owned()))
}
