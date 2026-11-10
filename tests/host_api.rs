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
                Fragment::Rule(_) | Fragment::DashedRule(_) | Fragment::Line { .. } => vec![],
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

use MathElement::{Math, Mfrac, Mi, Mn, Mo, Mrow, Mroot, Mspace, Msqrt, Msup, Mtable, Mtd, Mtr, Munderover};

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

/// Adapted from WPT `mathml/presentation-markup/radicals/root-parameters-1.html`. The WPT uses
/// fonts with made-up MATH constants; here the constants of the test font are used instead.
struct Radical {
    doc: Doc,
    root: Id,
    radical: Id,
    base: Id,
    index: Id,
}

fn space(doc: &mut Doc, width: &str, height: &str) -> Id {
    let id = doc.el(Mspace, &[]);
    doc.set_attr(id, "width", width);
    doc.set_attr(id, "height", height);
    id
}

fn radical(base_height: &str, index_width: &str, index_height: &str) -> Radical {
    let mut doc = Doc::new();
    let base = space(&mut doc, "3em", base_height);
    let index = space(&mut doc, index_width, index_height);
    let radical = doc.el(Mroot, &[base, index]);
    let root = doc.el(Math, &[radical]);
    doc.run(root);
    Radical { doc, root, radical, base, index }
}

impl Radical {
    fn rect(&self, node: Id) -> Rect {
        self.doc.absolute_rect(self.root, node)
    }
    fn constant(&self, c: math_render::host::MathConstant) -> f32 {
        use math_render::host::MathFont;
        let scale = 20.0 / self.doc.font.units_per_em() as f32;
        self.doc.font.math_constant(c) as f32 * scale
    }
}

#[test]
fn radical_kern_before_degree() {
    use math_render::host::MathConstant::RadicalKernBeforeDegree;
    let r = radical("1em", "3em", "1em");
    assert_close(
        r.rect(r.index).origin.x - r.rect(r.radical).origin.x,
        r.constant(RadicalKernBeforeDegree),
    );
}

#[test]
fn radical_kern_after_degree() {
    // the radicand moves by the width of the index, as long as the surd isn't pulled left
    let narrow = radical("2em", "7em", "1em");
    let wide = radical("2em", "8em", "1em");
    assert_close(
        wide.rect(wide.base).origin.x - narrow.rect(narrow.base).origin.x,
        20.0,
    );
}

#[test]
fn radical_degree_bottom_raise_percent() {
    use math_render::host::MathConstant::RadicalDegreeBottomRaisePercent;
    let mut r = radical("10em", "3em", "1em");
    // The bottom edge of the index, not its baseline, is raised: give it a depth.
    r.doc.set_attr(r.index, "depth", "0.5em");
    r.doc.run(r.root);
    let radical_rect = r.rect(r.radical);
    // The surd is the lowest thing in the radical, and its top is the top of the rule.
    let rule_top = r
        .doc
        .layout(r.radical)
        .fragments
        .iter()
        .find_map(|f| match f {
            Fragment::Rule(rect) => Some(rect.origin.y),
            _ => None,
        })
        .expect("the radical has a rule");
    let surd_height = radical_rect.bottom() - (radical_rect.origin.y + rule_top);
    let percent = r.doc.font.math_constant(RadicalDegreeBottomRaisePercent) as f32 / 100.0;
    let raise = radical_rect.bottom() - r.rect(r.index).bottom();
    assert_close(raise, percent * surd_height);
}

use MathElement::{Menclose, Mpadded};

fn padded(attrs: &[(&str, &str)]) -> (Doc, Id, Id) {
    let mut doc = Doc::new();
    let content = space(&mut doc, "3em", "1em");
    doc.set_attr(content, "depth", "0.5em");
    let padded = doc.el(Mpadded, &[content]);
    for (name, value) in attrs {
        doc.set_attr(padded, name, value);
    }
    let root = doc.el(Math, &[padded]);
    doc.run(root);
    (doc, root, padded)
}

#[test]
fn mpadded_sets_and_adjusts_dimensions() {
    let (doc, _, p) = padded(&[("width", "5em"), ("height", "+0.5em"), ("depth", "200%")]);
    let size = doc.layout(p).size;
    assert_close(size.width, 100.0);
    assert_close(size.height, 30.0 + 20.0);
    assert_close(doc.layout(p).baseline, 30.0);
}

#[test]
fn mpadded_pseudo_units() {
    let (doc, _, p) = padded(&[("width", "0.5width"), ("height", "2depth")]);
    assert_close(doc.layout(p).size.width, 30.0);
    assert_close(doc.layout(p).baseline, 20.0);
}

#[test]
fn mpadded_lspace_and_voffset_move_the_content() {
    let (doc, root, p) = padded(&[("lspace", "1em"), ("voffset", "0.5em")]);
    let plain = padded(&[]);
    let child = doc.nodes[p].children[0];
    let moved = doc.absolute_rect(root, child);
    let original = plain.0.absolute_rect(plain.1, plain.0.nodes[plain.2].children[0]);
    assert_close(moved.origin.x - original.origin.x, 20.0);
    assert_close(original.origin.y - moved.origin.y, 10.0);
    assert_close(doc.layout(p).size.width, 80.0);
    // voffset doesn't change the box
    assert_close(doc.layout(p).size.height, plain.0.layout(plain.2).size.height);
}

fn enclosed(notation: Option<&str>) -> (Doc, Id, Id) {
    let mut doc = Doc::new();
    let content = space(&mut doc, "3em", "1em");
    let enclose = doc.el(Menclose, &[content]);
    if let Some(notation) = notation {
        doc.set_attr(enclose, "notation", notation);
    }
    let root = doc.el(Math, &[enclose]);
    doc.run(root);
    (doc, enclose, content)
}

#[test]
fn menclose_box_draws_four_bars_around_the_content() {
    let (doc, enclose, content) = enclosed(Some("box"));
    let bars = doc
        .layout(enclose)
        .fragments
        .iter()
        .filter(|f| matches!(f, Fragment::Rule(_)))
        .count();
    assert_eq!(bars, 4);
    let outer = doc.layout(enclose).size;
    let inner = doc.layout(content).size;
    assert!(outer.width > inner.width && outer.height > inner.height);
}

#[test]
fn menclose_defaults_to_longdiv_and_strikes_add_no_space() {
    let (doc, enclose, _) = enclosed(None);
    assert_eq!(doc.layout(enclose).fragments.len(), 2);
    let (doc, enclose, content) = enclosed(Some("updiagonalstrike horizontalstrike"));
    assert_eq!(doc.layout(enclose).size, doc.layout(content).size);
    assert!(doc
        .layout(enclose)
        .fragments
        .iter()
        .any(|f| matches!(f, Fragment::Line { .. })));
}

#[test]
fn table_cells_span_columns() {
    let mut doc = Doc::new();
    let cell = |doc: &mut Doc, text| {
        let mn = doc.tok(Mn, text);
        doc.el(Mtd, &[mn])
    };
    let (a, b) = (cell(&mut doc, "1"), cell(&mut doc, "2"));
    let row1 = doc.el(Mtr, &[a, b]);
    // a wide cell in the second row spanning both columns
    let wide = space(&mut doc, "10em", "1em");
    let wide_cell = doc.el(Mtd, &[wide]);
    doc.set_attr(wide_cell, "columnspan", "2");
    let row2 = doc.el(Mtr, &[wide_cell]);
    let table = doc.el(Mtable, &[row1, row2]);
    doc.run(table);

    let rect = |n| doc.absolute_rect(table, n);
    // the columns grow to fit the spanning cell, which covers both of them
    assert_close(rect(wide_cell).size.width, rect(b).right() - rect(a).origin.x);
    assert!(rect(wide_cell).size.width >= 200.0);
}

#[test]
fn table_cells_span_rows() {
    let mut doc = Doc::new();
    let tall = space(&mut doc, "1em", "6em");
    let tall_cell = doc.el(Mtd, &[tall]);
    doc.set_attr(tall_cell, "rowspan", "2");
    let one = doc.tok(Mn, "1");
    let one = doc.el(Mtd, &[one]);
    let row1 = doc.el(Mtr, &[tall_cell, one]);
    // the second row's only cell goes to the second column
    let two = doc.tok(Mn, "2");
    let two = doc.el(Mtd, &[two]);
    let row2 = doc.el(Mtr, &[two]);
    let table = doc.el(Mtable, &[row1, row2]);
    doc.run(table);

    let rect = |n| doc.absolute_rect(table, n);
    assert_close(rect(two).origin.x, rect(one).origin.x);
    assert!(rect(two).origin.y > rect(one).origin.y);
    // the spanning cell is at least as tall as its content and covers both rows
    assert!(rect(tall_cell).size.height >= 120.0);
    assert_close(rect(tall_cell).origin.y.min(rect(one).origin.y), rect(tall_cell).origin.y);
    assert!(rect(tall_cell).bottom() >= rect(two).bottom() - 1e-3);
}

#[test]
fn table_lines_stop_at_spanning_cells() {
    let mut doc = Doc::new();
    let cell = |doc: &mut Doc, text| {
        let mn = doc.tok(Mn, text);
        doc.el(Mtd, &[mn])
    };
    let (a, b, c) = (cell(&mut doc, "1"), cell(&mut doc, "2"), cell(&mut doc, "3"));
    let row1 = doc.el(Mtr, &[a, b]);
    let wide = cell(&mut doc, "4");
    doc.set_attr(wide, "columnspan", "2");
    let row2 = doc.el(Mtr, &[wide]);
    let row3 = doc.el(Mtr, &[c]);
    let table = doc.el(Mtable, &[row1, row2, row3]);
    doc.set_attr(table, "columnlines", "solid");
    doc.run(table);

    let lines: Vec<Rect> = doc
        .layout(table)
        .fragments
        .iter()
        .filter_map(|f| match f {
            Fragment::Rule(rect) => Some(*rect),
            _ => None,
        })
        .collect();
    // the line between the columns is interrupted by the spanning cell in the second row
    assert_eq!(lines.len(), 2);
    let spanning = doc.absolute_rect(table, wide);
    for line in lines {
        assert!(line.bottom() <= spanning.origin.y + 1e-3 || line.origin.y >= spanning.bottom() - 1e-3);
    }
}

// Ports of the table tests in web-platform-tests `mathml/presentation-markup/tables/`. The
// reftests (`columnspan-rowspan-*`) become size and tiling checks, since mlayout has no
// rasterizer to compare against a reference page.

/// A `<mspace>` of the given size in px.
fn px_space(doc: &mut Doc, width: f32, height: f32, depth: f32) -> Id {
    let id = doc.el(Mspace, &[]);
    doc.set_attr(id, "width", &format!("{width}px"));
    doc.set_attr(id, "height", &format!("{height}px"));
    doc.set_attr(id, "depth", &format!("{depth}px"));
    id
}

/// A table of rows of cells; each cell is `(content, columnspan, rowspan)`. Returns the table,
/// the `mtd` of every cell (in reading order) and the table root.
fn spanned_table(doc: &mut Doc, rows: &[Vec<(Id, u32, u32)>]) -> (Id, Vec<Id>) {
    let mut cells = Vec::new();
    let mut row_ids = Vec::new();
    for row in rows {
        let mut row_cells = Vec::new();
        for &(content, column_span, row_span) in row {
            let mtd = doc.el(Mtd, &[content]);
            if column_span != 1 {
                doc.set_attr(mtd, "columnspan", &column_span.to_string());
            }
            if row_span != 1 {
                doc.set_attr(mtd, "rowspan", &row_span.to_string());
            }
            row_cells.push(mtd);
            cells.push(mtd);
        }
        row_ids.push(doc.el(Mtr, &row_cells));
    }
    let table = doc.el(Mtable, &row_ids);
    // MathML Core tables have no spacing between cells.
    doc.set_attr(table, "columnspacing", "0");
    doc.set_attr(table, "rowspacing", "0");
    (table, cells)
}

/// The table fills a `side` px square and its cells tile it without gaps or overlaps.
fn assert_green_square(doc: &mut Doc, table: Id, cells: &[Id], side: f32) {
    let root = doc.el(Math, &[table]);
    doc.run(root);
    let size = doc.layout(table).size;
    assert_close(size.width, side);
    assert_close(size.height, side);
    let mut area = 0.0;
    for (i, &a) in cells.iter().enumerate() {
        let ra = doc.absolute_rect(table, a);
        assert!(ra.origin.x >= -1e-3 && ra.origin.y >= -1e-3);
        assert!(ra.right() <= side + 1e-3 && ra.bottom() <= side + 1e-3);
        area += ra.size.width * ra.size.height;
        for &b in &cells[i + 1..] {
            let rb = doc.absolute_rect(table, b);
            let overlaps = ra.origin.x < rb.right() - 1e-3
                && rb.origin.x < ra.right() - 1e-3
                && ra.origin.y < rb.bottom() - 1e-3
                && rb.origin.y < ra.bottom() - 1e-3;
            assert!(!overlaps, "cells overlap: {ra:?} {rb:?}");
        }
    }
    assert_close(area, side * side);
}

fn squares(doc: &mut Doc, sizes: &[(f32, f32)]) -> Vec<Id> {
    sizes.iter().map(|&(w, d)| px_space(doc, w, 0.0, d)).collect()
}

/// table-001: cells of 2x2, 4x3 and 3x4 tables are in reading order.
#[test]
fn wpt_table_001_basic_layout() {
    for (columns, rows) in [(2, 2), (4, 3), (3, 4)] {
        let mut doc = Doc::new();
        let mut grid: Vec<Vec<Id>> = Vec::new();
        let mut trs = Vec::new();
        for _ in 0..rows {
            let mut spaces = Vec::new();
            let mut tds = Vec::new();
            for _ in 0..columns {
                let space = px_space(&mut doc, 20.0, 10.0, 0.0);
                tds.push(doc.el(Mtd, &[space]));
                spaces.push(space);
            }
            trs.push(doc.el(Mtr, &tds));
            grid.push(spaces);
        }
        let table = doc.el(Mtable, &trs);
        let root = doc.el(Math, &[table]);
        doc.run(root);
        let rect = |n| doc.absolute_rect(root, n);
        for j in 0..rows {
            for i in 0..columns - 1 {
                let (a, b) = (rect(grid[j][i]), rect(grid[j][i + 1]));
                assert_close(a.origin.y, b.origin.y);
                assert!(a.right() < b.origin.x, "({i},{j}) is left of ({},{j})", i + 1);
            }
        }
        for j in 0..rows - 1 {
            for i in 0..columns {
                let (a, b) = (rect(grid[j][i]), rect(grid[j + 1][i]));
                assert_close(a.origin.x, b.origin.x);
                assert!(a.bottom() < b.origin.y, "({i},{j}) is above ({i},{})", j + 1);
            }
        }
    }
}

/// table-002 and table-003: cells share a baseline and are centered in their column, with or
/// without an `<mrow>` around the content.
#[test]
fn wpt_table_002_003_cell_alignment() {
    for wrapped in [false, true] {
        let mut doc = Doc::new();
        let cells = [
            (40.0, 0.0),
            (0.0, 40.0),
            (20.0, 20.0),
            (30.0, 10.0),
            (10.0, 30.0),
            (80.0, 0.0),
            (0.0, 80.0),
            (40.0, 40.0),
            (70.0, 30.0),
            (30.0, 70.0),
        ];
        let mut spaces = Vec::new();
        let mut tds = Vec::new();
        for (height, depth) in cells {
            let space = px_space(&mut doc, 10.0, height, depth);
            let content = if wrapped { doc.el(Mrow, &[space]) } else { space };
            tds.push(doc.el(Mtd, &[content]));
            spaces.push(space);
        }
        let tr = doc.el(Mtr, &tds);
        let vertical = doc.el(Mtable, &[tr]);

        let widths = [10.0, 40.0, 30.0, 20.0, 50.0, 100.0, 90.0, 70.0, 80.0, 40.0];
        let mut wide = Vec::new();
        let mut trs = Vec::new();
        for width in widths {
            let space = px_space(&mut doc, width, 10.0, 0.0);
            let content = if wrapped { doc.el(Mrow, &[space]) } else { space };
            let td = doc.el(Mtd, &[content]);
            trs.push(doc.el(Mtr, &[td]));
            wide.push(space);
        }
        let horizontal = doc.el(Mtable, &trs);
        let root = doc.el(Math, &[vertical, horizontal]);
        doc.run(root);

        let baseline = |n| {
            let rect = doc.absolute_rect(root, n);
            rect.origin.y
        };
        for (i, (height, _)) in cells.iter().enumerate().take(9) {
            let next = cells[i + 1].0;
            assert!(
                (baseline(spaces[i]) + height - (baseline(spaces[i + 1]) + next)).abs() < 1.0,
                "baselines of cells {i} and {} are aligned (wrapped: {wrapped})",
                i + 1
            );
        }
        for pair in wide.windows(2) {
            let (a, b) = (doc.absolute_rect(root, pair[0]), doc.absolute_rect(root, pair[1]));
            assert!((a.origin.x + a.size.width / 2.0 - (b.origin.x + b.size.width / 2.0)).abs() < 1.0);
        }
    }
}

/// table-cell-mrow-layout: `<mtd>` lays out its children like `<mrow>`, and wrapping them in
/// an explicit `<mrow>` changes nothing.
#[test]
fn wpt_table_cell_mrow_layout() {
    let parts = [(10.0, 20.0, 20.0), (10.0, 30.0, 10.0), (10.0, 10.0, 30.0)];
    let mut doc = Doc::new();
    let mut direct = Vec::new();
    let mut wrapped = Vec::new();
    let mut reference = Vec::new();
    for &(width, height, depth) in &parts {
        direct.push(px_space(&mut doc, width, height, depth));
        wrapped.push(px_space(&mut doc, width, height, depth));
        reference.push(px_space(&mut doc, width, height, depth));
    }
    let mtd = doc.el(Mtd, &direct);
    let tr = doc.el(Mtr, &[mtd]);
    let mtable = doc.el(Mtable, &[tr]);
    let row = doc.el(Mrow, &wrapped);
    let mtd2 = doc.el(Mtd, &[row]);
    let tr2 = doc.el(Mtr, &[mtd2]);
    let mtable2 = doc.el(Mtable, &[tr2]);
    let mrow = doc.el(Mrow, &reference);
    let root = doc.el(Math, &[mtable, mtable2, mrow]);
    doc.run(root);

    let size = |n: Id| doc.layout(n).size;
    assert_close(size(mtd).width, size(mrow).width);
    assert_close(size(mtd).height, size(mrow).height);
    assert_close(doc.layout(mtd).baseline, doc.layout(mrow).baseline);
    assert_close(size(mtable).width, size(mtable2).width);
    assert_close(size(mtable).height, size(mtable2).height);
    for (a, b) in direct.iter().zip(&wrapped) {
        // positions inside the tables (the tables themselves sit side by side)
        let ra = doc.absolute_rect(root, *a);
        let rb = doc.absolute_rect(root, *b);
        let (ta, tb) = (doc.absolute_rect(root, mtable), doc.absolute_rect(root, mtable2));
        assert_close(ra.origin.x - ta.origin.x, rb.origin.x - tb.origin.x);
        assert_close(ra.origin.y - ta.origin.y, rb.origin.y - tb.origin.y);
    }
}

/// columnspan-rowspan-001: a cell spanning two columns.
#[test]
fn wpt_columnspan_rowspan_001() {
    let mut doc = Doc::new();
    let mut rows = Vec::new();
    let first = squares(&mut doc, &[(80.0, 40.0), (40.0, 40.0), (40.0, 40.0), (40.0, 40.0)]);
    rows.push(vec![(first[0], 2, 1), (first[1], 1, 1), (first[2], 1, 1), (first[3], 1, 1)]);
    for _ in 0..4 {
        let row = squares(&mut doc, &[(40.0, 40.0); 5]);
        rows.push(row.into_iter().map(|c| (c, 1, 1)).collect());
    }
    let (table, cells) = spanned_table(&mut doc, &rows);
    assert_green_square(&mut doc, table, &cells, 200.0);
}

/// columnspan-rowspan-002: a cell spanning three rows.
#[test]
fn wpt_columnspan_rowspan_002() {
    let mut doc = Doc::new();
    let mut rows = Vec::new();
    for (lead, count) in [(true, 3), (false, 3), (false, 3), (false, 4)] {
        let mut row = Vec::new();
        if lead && rows.is_empty() {
            let tall = px_space(&mut doc, 50.0, 0.0, 150.0);
            row.push((tall, 1, 3));
        }
        for _ in 0..count {
            row.push((px_space(&mut doc, 50.0, 0.0, 50.0), 1, 1));
        }
        rows.push(row);
    }
    let (table, cells) = spanned_table(&mut doc, &rows);
    assert_green_square(&mut doc, table, &cells, 200.0);
}

/// columnspan-rowspan-003: a cell spanning four rows and four columns.
#[test]
fn wpt_columnspan_rowspan_003() {
    let mut doc = Doc::new();
    let big = px_space(&mut doc, 160.0, 0.0, 160.0);
    let mut rows = vec![vec![(big, 4, 4), (px_space(&mut doc, 40.0, 0.0, 40.0), 1, 1)]];
    for _ in 0..3 {
        rows.push(vec![(px_space(&mut doc, 40.0, 0.0, 40.0), 1, 1)]);
    }
    rows.push((0..5).map(|_| (px_space(&mut doc, 40.0, 0.0, 40.0), 1, 1)).collect());
    let (table, cells) = spanned_table(&mut doc, &rows);
    assert_green_square(&mut doc, table, &cells, 200.0);
}

/// columnspan-rowspan-004: staircase of rowspans over a staircase of columnspans.
#[test]
fn wpt_columnspan_rowspan_004() {
    let mut doc = Doc::new();
    let mut rows = Vec::new();
    let first: Vec<(Id, u32, u32)> = (0..5)
        .map(|i| (px_space(&mut doc, 40.0, 0.0, 40.0 * (5 - i) as f32), 1, 5 - i as u32))
        .collect();
    rows.push(first);
    rows.push(vec![(px_space(&mut doc, 40.0, 0.0, 40.0), 1, 1)]);
    for span in 2..=4u32 {
        rows.push(vec![(px_space(&mut doc, 40.0 * span as f32, 0.0, 40.0), span, 1)]);
    }
    let (table, cells) = spanned_table(&mut doc, &rows);
    assert_green_square(&mut doc, table, &cells, 200.0);
}

/// columnspan-rowspan-005 and -006: staircases that meet; `rowspan="0"` means "to the last
/// row".
#[test]
fn wpt_columnspan_rowspan_005_006() {
    for zero in [false, true] {
        let mut doc = Doc::new();
        let mut rows = Vec::new();
        rows.push(vec![(px_space(&mut doc, 200.0, 0.0, 40.0), 5, 1)]);
        for k in 1..4u32 {
            let columns = 5 - k;
            let rows_spanned = 5 - k;
            let rowspan = if zero { 0 } else { rows_spanned };
            rows.push(vec![
                (px_space(&mut doc, 40.0 * columns as f32, 0.0, 40.0), columns, 1),
                (px_space(&mut doc, 40.0, 0.0, 40.0 * rows_spanned as f32), 1, rowspan),
            ]);
        }
        let last_span = if zero { 0 } else { 1 };
        rows.push(vec![
            (px_space(&mut doc, 40.0, 0.0, 40.0), 1, 1),
            (px_space(&mut doc, 40.0, 0.0, 40.0), 1, last_span),
        ]);
        let (table, cells) = spanned_table(&mut doc, &rows);
        assert_green_square(&mut doc, table, &cells, 200.0);
    }
}

use MathElement::Mlabeledtr;

/// A table with one `mlabeledtr` and one `mtr`; returns (table, label, cell of the labeled
/// row, cell of the plain row).
fn labeled_table(side: Option<&str>) -> (Doc, Id, Id, Id, Id) {
    let mut doc = Doc::new();
    let label_text = doc.tok(Mn, "(1)");
    let label = doc.el(Mtd, &[label_text]);
    let a = doc.tok(Mn, "a");
    let cell = doc.el(Mtd, &[a]);
    let labeled = doc.el(Mlabeledtr, &[label, cell]);
    let b = doc.tok(Mn, "b");
    let plain_cell = doc.el(Mtd, &[b]);
    let plain = doc.el(Mtr, &[plain_cell]);
    let table = doc.el(Mtable, &[labeled, plain]);
    if let Some(side) = side {
        doc.set_attr(table, "side", side);
    }
    let root = doc.el(Math, &[table]);
    doc.run(root);
    (doc, table, label, cell, plain_cell)
}

#[test]
fn labels_go_to_the_right_by_default() {
    let (doc, table, label, cell, plain_cell) = labeled_table(None);
    let rect = |n| doc.absolute_rect(table, n);
    assert!(rect(label).origin.x >= rect(cell).right());
    // the label is outside of the table's cells but inside its box
    assert_close(rect(label).right(), doc.layout(table).size.width);
    // the gap is at least minlabelspacing (0.8em), measured from the widest cell
    assert!(rect(label).origin.x - rect(cell).right() >= 16.0 - 1e-3);
    // cells don't move, labeled or not
    assert_close(rect(cell).origin.x, rect(plain_cell).origin.x);
    // the label shares the baseline of its row
    assert_close(rect(label).bottom() - rect(cell).bottom(), 0.0);
}

#[test]
fn labels_on_the_left_move_the_table_right() {
    let (doc, table, label, cell, _) = labeled_table(Some("left"));
    let rect = |n| doc.absolute_rect(table, n);
    assert_close(rect(label).origin.x, 0.0);
    assert!(rect(cell).origin.x >= rect(label).right() + 16.0 - 1e-3);
    let (right, ..) = labeled_table(None);
    assert_close(doc.layout(table).size.width, right.layout(right.nodes.len() - 2).size.width);
}

#[test]
fn minlabelspacing_sets_the_gap() {
    let (mut doc, table, label, cell, _) = labeled_table(None);
    doc.set_attr(table, "minlabelspacing", "50px");
    let root = doc.nodes.len() - 1;
    doc.run(root);
    let rect = |n| doc.absolute_rect(table, n);
    assert_close(rect(label).origin.x - rect(cell).right(), 50.0);
}

use MathElement::{Maligngroup, Malignmark};

fn group(doc: &mut Doc, align: &str) -> Id {
    let id = doc.el(Maligngroup, &[]);
    doc.set_attr(id, "groupalign", align);
    id
}

/// Rows `lhs = rhs` in one column: the left sides are right aligned, the right sides left
/// aligned, so that the `=` signs line up.
#[test]
fn alignment_groups_line_up_the_equals_signs() {
    let mut doc = Doc::new();
    let mut rows = Vec::new();
    let mut lhs = Vec::new();
    let mut equals = Vec::new();
    for (left, right) in [(10.0, 5.0), (40.0, 30.0), (25.0, 60.0)] {
        let (g1, g2) = (group(&mut doc, "right"), group(&mut doc, "left"));
        let l = px_space(&mut doc, left, 5.0, 0.0);
        let eq = doc.tok(Mo, "=");
        let r = px_space(&mut doc, right, 5.0, 0.0);
        let cell = doc.el(Mtd, &[g1, l, g2, eq, r]);
        rows.push(doc.el(Mtr, &[cell]));
        lhs.push(l);
        equals.push(eq);
    }
    let table = doc.el(Mtable, &rows);
    let root = doc.el(Math, &[table]);
    doc.run(root);

    let rect = |n| doc.absolute_rect(root, n);
    for pair in equals.windows(2) {
        assert_close(rect(pair[0]).origin.x, rect(pair[1]).origin.x);
    }
    for pair in lhs.windows(2) {
        assert_close(rect(pair[0]).right(), rect(pair[1]).right());
    }
    // the cells of the column are equally wide
    let widths: Vec<f32> = doc.nodes[table]
        .children
        .iter()
        .map(|&tr| doc.layout(doc.nodes[tr].children[0]).size.width)
        .collect();
    assert_close(widths[0], widths[1]);
    assert_close(widths[1], widths[2]);
}

#[test]
fn alignment_marks_override_groupalign() {
    let mut doc = Doc::new();
    let mut rows = Vec::new();
    let mut marks = Vec::new();
    for (before, after) in [(10.0, 20.0), (30.0, 5.0)] {
        let g = group(&mut doc, "left");
        let a = px_space(&mut doc, before, 5.0, 0.0);
        let mark = doc.el(Malignmark, &[]);
        let b = px_space(&mut doc, after, 5.0, 0.0);
        let cell = doc.el(Mtd, &[g, a, mark, b]);
        rows.push(doc.el(Mtr, &[cell]));
        marks.push(mark);
    }
    let table = doc.el(Mtable, &rows);
    let root = doc.el(Math, &[table]);
    doc.run(root);
    assert_close(
        doc.absolute_rect(root, marks[0]).origin.x,
        doc.absolute_rect(root, marks[1]).origin.x,
    );
}

#[test]
fn alignment_groups_work_inside_padded_cells() {
    let mut doc = Doc::new();
    let mut rows = Vec::new();
    let mut spaces = Vec::new();
    for width in [10.0, 40.0] {
        let g = group(&mut doc, "right");
        let space = px_space(&mut doc, width, 5.0, 0.0);
        let cell = doc.el(Mtd, &[g, space]);
        doc.style_mut(cell).padding = Edges { top: 1.0, right: 7.0, bottom: 1.0, left: 3.0 };
        rows.push(doc.el(Mtr, &[cell]));
        spaces.push(space);
    }
    let table = doc.el(Mtable, &rows);
    let root = doc.el(Math, &[table]);
    doc.run(root);
    let rect = |n| doc.absolute_rect(root, n);
    assert_close(rect(spaces[0]).right(), rect(spaces[1]).right());
    let cell_width = |tr: Id| doc.layout(doc.nodes[tr].children[0]).size.width;
    assert_close(cell_width(rows[0]), 40.0 + 10.0);
    assert_close(cell_width(rows[1]), 40.0 + 10.0);
}
