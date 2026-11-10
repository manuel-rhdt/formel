use std::collections::HashMap;
use std::path::Path as FsPath;

use crate::host::{Fragment, NodeLayout, Point};
use crate::standalone::{MathDocument, NodeId};

use ::svg::node::element::{Group, Line, Path, Rectangle};
use ::svg::node::Node;
use ::svg::Document;

use std::fmt::Write;

use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};
use rustybuzz::Face;

/// Space added around the layout on every side, in px.
pub const MARGIN: f32 = 10.0;

#[derive(Debug, Default, Clone, Copy)]
pub struct Flags {
    /// Fill the boxes of token elements.
    pub show_ink_bounds: bool,
    /// Outline the border box of every element.
    pub show_logical_bounds: bool,
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
    let svg_document = build_document(document, layouts, font, units_per_em, flags);
    ::svg::save(out_path, &svg_document).unwrap();
}

/// Renders a laid-out document to an SVG string. Coordinates are px; `units_per_em` converts
/// glyph outlines from font units.
pub fn render_to_string(
    document: &MathDocument,
    layouts: &HashMap<NodeId, NodeLayout>,
    font: &Face<'_>,
    units_per_em: f32,
    flags: Flags,
) -> String {
    build_document(document, layouts, font, units_per_em, flags).to_string()
}

fn build_document(
    document: &MathDocument,
    layouts: &HashMap<NodeId, NodeLayout>,
    font: &Face<'_>,
    units_per_em: f32,
    flags: Flags,
) -> Document {
    let root = document.root().expect("document has a root");
    let root_layout = &layouts[&root];

    let mut svg_document = Document::new();
    svg_document.assign(
        "viewBox",
        (
            -MARGIN,
            -MARGIN,
            root_layout.size.width + 2.0 * MARGIN,
            root_layout.size.height + 2.0 * MARGIN,
        ),
    );

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
