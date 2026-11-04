//! Tests of the host API with a small mock host: the host's CSS must be the source of truth.

use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;

use harfbuzz_rs::{Face, Font};
use math_render::host::{
    Edges, Fragment, MathComputedStyle, MathElement, MathLayoutHost, MathShift, MathStyle,
    MathTree, NodeKind, NodeLayout, Rect, axis_height, layout_math,
};
use math_render::shaper::HarfbuzzShaper;

mod util;

type Id = usize;

struct Node {
    element: MathElement,
    children: Vec<Id>,
    style: MathComputedStyle,
    text: Option<String>,
    attributes: HashMap<String, String>,
}

/// A minimal host document. Styles are set explicitly per node, as a CSS engine would compute
/// them.
struct Doc {
    nodes: Vec<Node>,
    font: Rc<HarfbuzzShaper<'static>>,
    layouts: HashMap<Id, NodeLayout>,
    set_layout_calls: Vec<Id>,
}

fn test_font() -> Rc<HarfbuzzShaper<'static>> {
    let face = Face::new(util::get_bytes(), 0);
    Rc::new(HarfbuzzShaper::new(Font::new(face).into()))
}

impl Doc {
    fn new() -> Self {
        Doc {
            nodes: Vec::new(),
            font: test_font(),
            layouts: HashMap::new(),
            set_layout_calls: Vec::new(),
        }
    }

    fn add(&mut self, element: MathElement, children: &[Id], text: Option<&str>) -> Id {
        self.nodes.push(Node {
            element,
            children: children.to_vec(),
            style: MathComputedStyle {
                font_size: 20.0,
                ..Default::default()
            },
            text: text.map(str::to_owned),
            attributes: HashMap::new(),
        });
        self.nodes.len() - 1
    }

    fn el(&mut self, element: MathElement, children: &[Id]) -> Id {
        self.add(element, children, None)
    }

    fn tok(&mut self, element: MathElement, text: &str) -> Id {
        self.add(element, &[], Some(text))
    }

    fn style_mut(&mut self, node: Id) -> &mut MathComputedStyle {
        &mut self.nodes[node].style
    }

    fn set_attr(&mut self, node: Id, name: &str, value: &str) {
        self.nodes[node]
            .attributes
            .insert(name.to_owned(), value.to_owned());
    }

    /// Applies `f` to the style of `node` and all its descendants.
    fn style_subtree(&mut self, node: Id, f: &dyn Fn(&mut MathComputedStyle)) {
        f(&mut self.nodes[node].style);
        for child in self.nodes[node].children.clone() {
            self.style_subtree(child, f);
        }
    }

    fn run(&mut self, root: Id) -> NodeLayout {
        self.layouts.clear();
        self.set_layout_calls.clear();
        layout_math(self, root)
    }

    fn layout(&self, node: Id) -> &NodeLayout {
        &self.layouts[&node]
    }

    fn glyph_ids(&self, node: Id) -> Vec<u32> {
        self.layout(node)
            .fragments
            .iter()
            .flat_map(|f| match f {
                Fragment::Glyphs { glyphs, .. } => glyphs.iter().map(|g| g.glyph).collect(),
                Fragment::Rule(_) | Fragment::DashedRule(_) => vec![],
            })
            .collect()
    }

    /// The border box of `node` relative to the root's border box.
    fn absolute_rect(&self, root: Id, node: Id) -> Rect {
        let mut path = vec![node];
        while *path.last().unwrap() != root {
            let current = *path.last().unwrap();
            let parent = self
                .nodes
                .iter()
                .position(|n| n.children.contains(&current))
                .expect("node is in the tree");
            path.push(parent);
        }
        let (mut x, mut y) = (0.0, 0.0);
        for &n in &path {
            x += self.layout(n).location.x;
            y += self.layout(n).location.y;
        }
        let size = self.layout(node).size;
        Rect::new(x, y, size.width, size.height)
    }
}

impl MathTree for Doc {
    type NodeId = Id;
    type Font = Rc<HarfbuzzShaper<'static>>;

    fn kind(&self, node: Id) -> NodeKind {
        NodeKind::Math(self.nodes[node].element)
    }
    fn children(&self, node: Id) -> impl Iterator<Item = Id> {
        self.nodes[node].children.iter().copied()
    }
    fn style(&self, node: Id) -> MathComputedStyle {
        self.nodes[node].style
    }
    fn font(&self, _node: Id) -> Self::Font {
        self.font.clone()
    }
    fn text(&self, node: Id) -> Option<Cow<'_, str>> {
        self.nodes[node].text.as_deref().map(Cow::Borrowed)
    }
    fn attribute(&self, node: Id, name: &str) -> Option<Cow<'_, str>> {
        self.nodes[node]
            .attributes
            .get(name)
            .map(|v| Cow::Borrowed(v.as_str()))
    }
}

impl MathLayoutHost for Doc {
    fn set_layout(&mut self, node: Id, layout: NodeLayout) {
        self.set_layout_calls.push(node);
        self.layouts.insert(node, layout);
    }
}

fn assert_close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{} != {}", a, b);
}

use MathElement::{Math, Mfrac, Mi, Mn, Mo, Mrow, Msqrt, Msup, Mtable, Mtd, Mtr, Munderover};

#[test]
fn padding_and_border_inflate_the_box() {
    let mut doc = Doc::new();
    let a = doc.tok(Mi, "a");
    let b = doc.tok(Mi, "b");
    let row = doc.el(Mrow, &[a, b]);

    doc.run(row);
    let plain_row_width = doc.layout(row).size.width;
    let plain_a = doc.layout(a).clone();
    let plain_b_x = doc.layout(b).location.x;

    doc.style_mut(a).padding = Edges {
        top: 1.0,
        right: 4.0,
        bottom: 2.0,
        left: 3.0,
    };
    doc.style_mut(a).border = Edges {
        top: 0.0,
        right: 2.0,
        bottom: 0.0,
        left: 1.0,
    };
    doc.run(row);

    let extra = 3.0 + 4.0 + 1.0 + 2.0;
    // Without padding, `b` follows `a`'s italic correction. Right padding replaces it.
    let italic_correction = plain_b_x - plain_a.size.width;
    assert_close(doc.layout(row).size.width, plain_row_width + extra - italic_correction);
    assert_close(doc.layout(a).size.width, plain_a.size.width + extra);
    assert_close(doc.layout(a).size.height, plain_a.size.height + 3.0);
    assert_close(doc.layout(b).location.x, plain_a.size.width + extra);
    // the glyphs sit inside the padding and border
    match &doc.layout(a).fragments[0] {
        Fragment::Glyphs { origin, .. } => assert_close(origin.x, 3.0 + 1.0),
        other => panic!("unexpected fragment {:?}", other),
    }

    // margins move the box and its siblings, and are not part of the border box
    doc.style_mut(b).margin.left = 5.0;
    doc.run(row);
    assert_close(doc.layout(b).location.x, plain_a.size.width + extra + 5.0);
    assert_close(
        doc.layout(row).size.width,
        plain_row_width + extra - italic_correction + 5.0,
    );
}

#[test]
fn font_size_comes_from_css() {
    let mut doc = Doc::new();
    let x = doc.tok(Mi, "x");
    let two = doc.tok(Mn, "2");
    let row = doc.el(Mrow, &[x, two]);

    doc.run(row);
    let x_width = doc.layout(x).size.width;
    let two_width = doc.layout(two).size.width;

    doc.style_mut(x).font_size = 40.0;
    doc.run(row);
    assert_close(doc.layout(x).size.width, 2.0 * x_width);
    assert_close(doc.layout(two).size.width, two_width);
    match &doc.layout(x).fragments[0] {
        Fragment::Glyphs { font_size, .. } => assert_close(*font_size, 40.0),
        other => panic!("unexpected fragment {:?}", other),
    }
}

#[test]
fn math_depth_selects_script_glyphs_without_scaling() {
    let mut doc = Doc::new();
    let prime = doc.tok(Mi, "\u{2032}");
    let row = doc.el(Mrow, &[prime]);

    doc.run(row);
    let normal_glyphs = doc.glyph_ids(prime);

    doc.style_mut(prime).math_depth = 1;
    doc.run(row);
    assert_ne!(doc.glyph_ids(prime), normal_glyphs, "ssty variant expected");
    match &doc.layout(prime).fragments[0] {
        Fragment::Glyphs { font_size, .. } => assert_close(*font_size, 20.0),
        other => panic!("unexpected fragment {:?}", other),
    }
}

#[test]
fn math_style_controls_fractions() {
    let mut doc = Doc::new();
    let one = doc.tok(Mn, "1");
    let two = doc.tok(Mn, "2");
    let frac = doc.el(Mfrac, &[one, two]);

    doc.run(frac);
    let display_height = doc.layout(frac).size.height;
    assert_eq!(
        doc.layout(frac)
            .fragments
            .iter()
            .filter(|f| matches!(f, Fragment::Rule(_)))
            .count(),
        1,
        "the fraction bar belongs to the mfrac"
    );

    doc.style_mut(frac).math_style = MathStyle::Compact;
    doc.run(frac);
    assert!(doc.layout(frac).size.height < display_height);

    // linethickness="0" draws a stack without a bar
    doc.set_attr(frac, "linethickness", "0");
    doc.run(frac);
    assert!(doc.layout(frac).fragments.is_empty());
}

#[test]
fn movable_limits_follow_math_style() {
    let mut doc = Doc::new();
    let sum = doc.tok(Mo, "\u{2211}");
    let lower = doc.tok(Mi, "i");
    let upper = doc.tok(Mi, "n");
    let underover = doc.el(Munderover, &[sum, lower, upper]);
    for script in [lower, upper] {
        doc.style_mut(script).math_depth = 1;
        doc.style_mut(script).font_size = 14.2;
    }

    doc.run(underover);
    let sum_rect = doc.absolute_rect(underover, sum);
    let lower_rect = doc.absolute_rect(underover, lower);
    assert!(lower_rect.origin.y >= sum_rect.bottom(), "limit below the sum");

    doc.style_subtree(underover, &|s| s.math_style = MathStyle::Compact);
    doc.run(underover);
    let sum_rect = doc.absolute_rect(underover, sum);
    let lower_rect = doc.absolute_rect(underover, lower);
    assert!(lower_rect.origin.x >= sum_rect.right() - 1.0, "limit becomes a subscript");
}

#[test]
fn math_shift_uses_cramped_superscripts() {
    let mut doc = Doc::new();
    let x = doc.tok(Mi, "x");
    let two = doc.tok(Mn, "2");
    let sup = doc.el(Msup, &[x, two]);
    doc.style_mut(two).math_depth = 1;

    doc.run(sup);
    let normal_y = doc.absolute_rect(sup, two).origin.y - doc.absolute_rect(sup, x).origin.y;

    doc.style_mut(sup).math_shift = MathShift::Compact;
    doc.run(sup);
    let cramped_y = doc.absolute_rect(sup, two).origin.y - doc.absolute_rect(sup, x).origin.y;
    assert!(cramped_y > normal_y, "cramped superscripts are raised less");
}

#[test]
fn stretchy_operators_follow_overrides() {
    fn paren_height(stretchy: Option<&str>) -> f32 {
        let mut doc = Doc::new();
        let open = doc.tok(Mo, "(");
        let close = doc.tok(Mo, ")");
        let num = doc.tok(Mi, "a");
        let den = doc.tok(Mi, "b");
        let inner_num = doc.el(Mfrac, &[num, den]);
        let one = doc.tok(Mn, "1");
        let frac = doc.el(Mfrac, &[inner_num, one]);
        let row = doc.el(Mrow, &[open, frac, close]);
        if let Some(value) = stretchy {
            doc.set_attr(open, "stretchy", value);
        }
        doc.run(row);
        doc.layout(open).size.height
    }

    let mut doc = Doc::new();
    let open = doc.tok(Mo, "(");
    let row = doc.el(Mrow, &[open]);
    doc.run(row);
    let natural = doc.layout(open).size.height;

    assert!(paren_height(None) > natural * 1.5, "paren stretches");
    assert_close(paren_height(Some("false")), natural);
}

#[test]
fn every_node_is_reported_once_and_nested() {
    let mut doc = Doc::new();
    let a = doc.tok(Mi, "a");
    let two = doc.tok(Mn, "2");
    let a2 = doc.el(Msup, &[a, two]);
    let plus = doc.tok(Mo, "+");
    let b = doc.tok(Mi, "b");
    let sqrt = doc.el(Msqrt, &[b]);
    let num = doc.el(Mrow, &[a2, plus, sqrt]);
    let c = doc.tok(Mi, "c");
    let frac = doc.el(Mfrac, &[num, c]);
    let math = doc.el(Math, &[frac]);
    doc.style_mut(two).math_depth = 1;
    doc.style_mut(sqrt).padding = Edges::uniform(1.0);

    let root_layout = doc.run(math);
    assert_eq!(root_layout.location.x, 0.0);
    assert_eq!(root_layout.location.y, 0.0);

    let mut calls = doc.set_layout_calls.clone();
    calls.sort();
    assert_eq!(calls, (0..doc.nodes.len()).collect::<Vec<_>>());

    for parent in 0..doc.nodes.len() {
        let outer = doc.absolute_rect(math, parent);
        for &child in &doc.nodes[parent].children {
            let inner = doc.absolute_rect(math, child);
            let eps = 1e-3;
            assert!(
                inner.origin.x >= outer.origin.x - eps
                    && inner.origin.y >= outer.origin.y - eps
                    && inner.right() <= outer.right() + eps
                    && inner.bottom() <= outer.bottom() + eps,
                "node {} {:?} not inside parent {} {:?}",
                child,
                inner,
                parent,
                outer
            );
        }
    }
}

/// A table of `mn` cells; returns (table, rows of (row, cells)).
fn number_table(doc: &mut Doc, rows: &[&[&str]]) -> (Id, Vec<(Id, Vec<Id>)>) {
    let mut row_ids = Vec::new();
    for row in rows {
        let mut cells = Vec::new();
        for text in *row {
            let mn = doc.tok(Mn, text);
            cells.push(doc.el(Mtd, &[mn]));
        }
        let tr = doc.el(Mtr, &cells);
        row_ids.push((tr, cells));
    }
    let table = doc.el(Mtable, &row_ids.iter().map(|(r, _)| *r).collect::<Vec<_>>());
    (table, row_ids)
}

#[test]
fn table_cells_form_columns_and_rows() {
    let mut doc = Doc::new();
    let (table, rows) = number_table(&mut doc, &[&["1", "22"], &["333", "4"]]);
    doc.run(table);

    let rect = |doc: &Doc, n| doc.absolute_rect(table, n);
    let (a, b) = (rows[0].1[0], rows[0].1[1]);
    let (c, d) = (rows[1].1[0], rows[1].1[1]);
    // cells fill their column (same x and width) and their row (same y and height)
    assert_close(rect(&doc, a).origin.x, rect(&doc, c).origin.x);
    assert_close(rect(&doc, a).size.width, rect(&doc, c).size.width);
    assert_close(rect(&doc, b).origin.x, rect(&doc, d).origin.x);
    assert_close(rect(&doc, a).origin.y, rect(&doc, b).origin.y);
    assert_close(rect(&doc, a).size.height, rect(&doc, b).size.height);
    // the first column is as wide as "333"
    let wide = doc.layout(doc.nodes[c].children[0]).size.width;
    assert_close(rect(&doc, a).size.width, wide);
    // rows span the table
    assert_close(doc.layout(rows[0].0).size.width, doc.layout(table).size.width);

    // centered on the math axis
    let layout = doc.layout(table);
    let center_above_baseline = layout.baseline - layout.size.height / 2.0;
    assert_close(center_above_baseline, axis_height(&doc, table));
}

#[test]
fn table_column_alignment() {
    let mut doc = Doc::new();
    let (table, rows) = number_table(&mut doc, &[&["1"], &["333"]]);
    let narrow = doc.nodes[rows[0].1[0]].children[0];

    doc.run(table);
    let centered_x = doc.absolute_rect(table, narrow).origin.x;

    doc.set_attr(table, "columnalign", "left");
    doc.run(table);
    let left_x = doc.absolute_rect(table, narrow).origin.x;
    assert_close(left_x, doc.absolute_rect(table, rows[0].1[0]).origin.x);
    assert!(left_x < centered_x);

    // a cell attribute overrides the table
    doc.set_attr(rows[0].1[0], "columnalign", "right");
    doc.run(table);
    let cell = doc.absolute_rect(table, rows[0].1[0]);
    let content = doc.absolute_rect(table, narrow);
    assert_close(content.right(), cell.right());
}

#[test]
fn table_lines_and_frame() {
    let mut doc = Doc::new();
    let (table, _) = number_table(&mut doc, &[&["1", "2", "3"], &["4", "5", "6"]]);
    doc.set_attr(table, "columnlines", "solid none");
    doc.set_attr(table, "rowlines", "dashed");
    doc.set_attr(table, "frame", "solid");
    doc.run(table);

    let fragments = &doc.layout(table).fragments;
    let solid = fragments
        .iter()
        .filter(|f| matches!(f, Fragment::Rule(_)))
        .count();
    let dashed = fragments
        .iter()
        .filter(|f| matches!(f, Fragment::DashedRule(_)))
        .count();
    assert_eq!(solid, 1 + 4, "one column line and four frame sides");
    assert_eq!(dashed, 1, "one row line");
}

#[test]
fn fences_stretch_around_tables() {
    let mut doc = Doc::new();
    let open = doc.tok(Mo, "(");
    let close = doc.tok(Mo, ")");
    let (table, _) = number_table(&mut doc, &[&["1"], &["2"], &["3"]]);
    let row = doc.el(Mrow, &[open, table, close]);
    doc.run(row);

    let table_rect = doc.absolute_rect(row, table);
    let open_rect = doc.absolute_rect(row, open);
    assert!(open_rect.size.height >= table_rect.size.height * 0.9);
    assert!(open_rect.origin.y <= table_rect.origin.y + 1.0);
}
