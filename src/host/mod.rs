//! Math layout on behalf of a host HTML/CSS layout engine.
//!
//! The host exposes its document through [`MathTree`]: the structure, the computed CSS of each
//! node ([`MathComputedStyle`]) and fonts ([`MathFont`]). [`layout_math`] then runs the MathML
//! layout algorithms directly over that tree. Every node's result (border box, baseline, and
//! the glyphs and rules it paints) goes back through [`MathLayoutHost::set_layout`].
//!
//! CSS is the source of truth: `font-size` (including `font-size: math`), `math-depth`,
//! `math-style`, `math-shift`, padding, border and margin are all read from the host and never
//! derived by mlayout. All lengths are CSS px.

mod font;
mod geometry;
mod output;
mod style;
mod tree;

use std::borrow::Cow;
use std::collections::HashMap;

pub use self::font::{
    CornerPosition, Extents, GlyphCode, MathConstant, MathFont, MathGlyph, ShapeOptions, Vector,
};
pub use self::geometry::{Edges, Point, Rect, Size};
pub use self::output::{ForeignMetrics, Fragment, NodeLayout, PositionedGlyph};
pub use self::style::{
    Direction, Form, MathComputedStyle, MathShift, MathStyle, OperatorOverrides,
    parse_math_length, script_scale_factor,
};
pub use self::tree::{
    MathElement, MathLayoutHost, MathTree, NodeKind, default_operator_overrides,
};
pub use crate::typesetting::engine::{intrinsic_inline_size, layout_math};

/// The height of the math axis above the baseline of `node`, in px: the font's `AxisHeight`
/// at the node's `font-size`. Fractions, stretchy operators and tables are centered on it.
pub fn axis_height<T: MathTree + ?Sized>(tree: &T, node: T::NodeId) -> f32 {
    let font = tree.font(node);
    let scale = tree.style(node).font_size / font.units_per_em().max(1) as f32;
    font.constant(MathConstant::AxisHeight) as f32 * scale
}

/// Wraps a [`MathTree`] and collects the layout results in a map, for hosts that would rather
/// not implement [`MathLayoutHost`] themselves.
///
/// ```ignore
/// let mut collector = LayoutCollector::new(&my_tree);
/// layout_math(&mut collector, root);
/// let layout = &collector.layouts[&some_node];
/// ```
#[derive(Debug)]
pub struct LayoutCollector<T: MathTree> {
    pub tree: T,
    pub layouts: HashMap<T::NodeId, NodeLayout>,
}

impl<T: MathTree> LayoutCollector<T> {
    pub fn new(tree: T) -> Self {
        LayoutCollector {
            tree,
            layouts: HashMap::new(),
        }
    }

    pub fn get(&self, node: T::NodeId) -> Option<&NodeLayout> {
        self.layouts.get(&node)
    }
}

impl<T: MathTree> MathTree for LayoutCollector<T> {
    type NodeId = T::NodeId;
    type Font = T::Font;

    fn kind(&self, node: Self::NodeId) -> NodeKind {
        self.tree.kind(node)
    }
    fn children(&self, node: Self::NodeId) -> impl Iterator<Item = Self::NodeId> {
        self.tree.children(node)
    }
    fn child_count(&self, node: Self::NodeId) -> usize {
        self.tree.child_count(node)
    }
    fn style(&self, node: Self::NodeId) -> MathComputedStyle {
        self.tree.style(node)
    }
    fn font(&self, node: Self::NodeId) -> Self::Font {
        self.tree.font(node)
    }
    fn text(&self, node: Self::NodeId) -> Option<Cow<'_, str>> {
        self.tree.text(node)
    }
    fn attribute(&self, node: Self::NodeId, name: &str) -> Option<Cow<'_, str>> {
        self.tree.attribute(node, name)
    }
    fn operator_overrides(&self, node: Self::NodeId) -> OperatorOverrides {
        self.tree.operator_overrides(node)
    }
}

impl<T: MathTree> MathLayoutHost for LayoutCollector<T> {
    fn set_layout(&mut self, node: Self::NodeId, layout: NodeLayout) {
        self.layouts.insert(node, layout);
    }
}
