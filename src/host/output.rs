//! The layout results handed back to the host.

use super::font::GlyphCode;
use super::geometry::{Point, Rect, Size};

/// The margin-box metrics of foreign content, measured from its baseline.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ForeignMetrics {
    pub width: f32,
    /// Distance from the top of the margin box to the baseline.
    pub ascent: f32,
    /// Distance from the baseline to the bottom of the margin box.
    pub descent: f32,
}

/// The layout of one node.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct NodeLayout {
    /// The top left corner of the border box, relative to the parent's border box. The node
    /// passed to [`layout_math`](super::layout_math) gets `(0, 0)`.
    pub location: Point,
    /// The size of the border box.
    pub size: Size,
    /// Distance from the top of the border box to the alphabetic baseline.
    pub baseline: f32,
    /// What this node paints itself, relative to its border box. Fragments of descendant nodes
    /// are reported on those nodes.
    pub fragments: Vec<Fragment>,
}

/// One painted item.
#[derive(Debug, Clone, PartialEq)]
pub enum Fragment {
    /// A run of glyphs from the node's font, drawn at `font_size`.
    Glyphs {
        /// The start of the run on its baseline.
        origin: Point,
        font_size: f32,
        glyphs: Vec<PositionedGlyph>,
    },
    /// A filled rectangle, e.g. a fraction bar, the overbar of a radical or a solid table
    /// line. It is painted in the node's text color.
    Rule(Rect),
    /// A dashed table line (`columnlines`, `rowlines` or `frame` set to `dashed`). The rectangle
    /// is the line's extent; it runs along its longer side. The dash pattern is up to the host
    /// (e.g. dashes three times as long as the line is thick).
    DashedRule(Rect),
}

/// A glyph positioned relative to the origin of its [`Fragment::Glyphs`] run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionedGlyph {
    pub glyph: GlyphCode,
    /// Offset of the glyph's origin from the run origin. The y axis points downwards.
    pub offset: Point,
    /// Byte offset into the node's text that produced this glyph.
    pub cluster: u32,
}
