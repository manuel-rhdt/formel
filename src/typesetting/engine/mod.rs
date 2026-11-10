//! The layout engine behind [`crate::host::layout_math`]. It runs the MathML layout algorithms
//! directly over a host tree.

mod enclose;
mod fraction;
mod lbox;
mod operator;
mod row;
mod scripts;
mod table;

use crate::host::{
    Edges, Form, Fragment, MathComputedStyle, MathElement, MathLayoutHost, MathTree, NodeKind,
    NodeLayout, Point, PositionedGlyph, Rect, ShapeOptions, Size,
};

use self::lbox::{Content, LBox, Metrics, ScaledFont};

/// Lays out the MathML subtree rooted at `root`, usually a `<math>` element.
///
/// [`MathLayoutHost::set_layout`] is called for every laid-out node, including `root`, whose
/// layout is also returned. `root`'s margins are ignored: placing the root box is the host's
/// job (e.g. as an atomic inline, or centered with `margin: auto` for `display: block math`).
pub fn layout_math<T: MathLayoutHost>(tree: &mut T, root: T::NodeId) -> NodeLayout {
    let mut engine = Engine { tree };
    let root_box = engine.layout_node_with_margin(root, Ctx::default(), false);
    let border_origin = Point::new(0.0, -root_box.metrics.ascent);
    engine
        .emit(root_box, Point::ZERO, border_origin, &mut Vec::new(), false, true)
        .expect("the root box belongs to the root node")
}

/// The inline size (width of the border box) of `node`. Math content has no line breaks, so
/// this is both the min-content and the max-content size. Nothing is reported to
/// [`MathLayoutHost::set_layout`].
pub fn intrinsic_inline_size<T: MathLayoutHost>(tree: &mut T, node: T::NodeId) -> f32 {
    let mut engine = Engine { tree };
    engine
        .layout_node_with_margin(node, Ctx::default(), false)
        .metrics
        .advance
}

pub(crate) struct Engine<'t, T: MathLayoutHost> {
    pub(crate) tree: &'t mut T,
}

pub(crate) type Box_<T> = LBox<<T as MathTree>::NodeId>;

/// The size a stretchy operator should cover.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct StretchSize {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
}

/// Layout state passed from a parent to a child. It is not part of CSS: it carries
/// information from the surrounding layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ctx {
    /// Target size if the node is (or embellishes) a stretchy operator.
    pub stretch: Option<StretchSize>,
    /// The node is the accent of an `<mover>` (not of an `<munder>`).
    pub accent_over: bool,
    /// Use flattened accent glyphs.
    pub flat_accent: bool,
    /// Default operator form, from the node's position among its siblings.
    pub form: Form,
}

impl Default for Ctx {
    fn default() -> Self {
        Ctx {
            stretch: None,
            accent_over: false,
            flat_accent: false,
            form: Form::Infix,
        }
    }
}

impl Ctx {
    /// The context for a child that is not on the path to an embellished operator's core.
    pub fn plain(form: Form) -> Self {
        Ctx {
            form,
            ..Ctx::default()
        }
    }
}

impl<'t, T: MathLayoutHost> Engine<'t, T> {
    pub(crate) fn children(&self, node: T::NodeId) -> Vec<T::NodeId> {
        self.tree.children(node).collect()
    }

    pub(crate) fn font(&self, node: T::NodeId, style: &MathComputedStyle) -> ScaledFont<T::Font> {
        ScaledFont::new(self.tree.font(node), style.font_size)
    }

    pub(crate) fn shape_options(style: &MathComputedStyle, ctx: Ctx) -> ShapeOptions {
        ShapeOptions {
            script_level: style.math_depth.clamp(0, 2) as u8,
            flat_accent: ctx.flat_accent,
        }
    }

    pub(crate) fn layout_node(&mut self, node: T::NodeId, ctx: Ctx) -> Box_<T> {
        self.layout_node_with_margin(node, ctx, true)
    }

    fn layout_node_with_margin(
        &mut self,
        node: T::NodeId,
        ctx: Ctx,
        include_margin: bool,
    ) -> Box_<T> {
        let element = match self.tree.kind(node) {
            NodeKind::Math(element) => element,
            NodeKind::Foreign => {
                let metrics = self.tree.layout_foreign(node);
                let mut foreign = LBox::empty(metrics.width, metrics.ascent, metrics.descent);
                if include_margin {
                    foreign.margin = self.tree.style(node).margin;
                }
                foreign.node = Some(node);
                return foreign;
            }
        };
        let style = self.tree.style(node);
        let content = self.layout_element(node, element, &style, ctx);
        let mut node_box = inflate(content, &style, include_margin);
        node_box.node = Some(node);
        node_box.phantom |= element == MathElement::Mphantom;
        node_box
    }

    fn layout_element(
        &mut self,
        node: T::NodeId,
        element: MathElement,
        style: &MathComputedStyle,
        ctx: Ctx,
    ) -> Box_<T> {
        use MathElement::*;
        let children = if element.is_token() {
            Vec::new()
        } else {
            self.children(node)
        };
        match element {
            Mi | Mn | Mtext => {
                let text = self.tree.text(node).unwrap_or_default().into_owned();
                self.layout_text(node, &text, style, ctx)
            }
            Ms => {
                let quote = |name| {
                    self.tree
                        .attribute(node, name)
                        .map_or_else(|| "\"".to_owned(), |q| q.into_owned())
                };
                let text = format!(
                    "{}{}{}",
                    quote("lquote"),
                    self.tree.text(node).unwrap_or_default(),
                    quote("rquote")
                );
                self.layout_text(node, &text, style, ctx)
            }
            Mo => self.layout_operator(node, style, ctx),
            Mspace => self.layout_space(node, style),
            Mfrac if children.len() == 2 => {
                self.layout_fraction(node, children[0], children[1], style, ctx)
            }
            Msqrt => self.layout_radical(node, None, &children, style),
            Mroot if children.len() == 2 => {
                self.layout_radical(node, Some(children[1]), &children[..1], style)
            }
            Msub if children.len() == 2 => {
                self.layout_scripts(node, children[0], Some(children[1]), None, style, ctx)
            }
            Msup if children.len() == 2 => {
                self.layout_scripts(node, children[0], None, Some(children[1]), style, ctx)
            }
            Msubsup if children.len() == 3 => self.layout_scripts(
                node,
                children[0],
                Some(children[1]),
                Some(children[2]),
                style,
                ctx,
            ),
            Munder if children.len() == 2 => {
                self.layout_under_over(node, children[0], Some(children[1]), None, style, ctx)
            }
            Mover if children.len() == 2 => {
                self.layout_under_over(node, children[0], None, Some(children[1]), style, ctx)
            }
            Munderover if children.len() == 3 => self.layout_under_over(
                node,
                children[0],
                Some(children[1]),
                Some(children[2]),
                style,
                ctx,
            ),
            Mmultiscripts if !children.is_empty() => {
                self.layout_multiscripts(node, &children, style, ctx)
            }
            Mpadded => self.layout_padded(node, &children, style, ctx),
            Menclose => self.layout_enclose(node, &children, style, ctx),
            Semantics => self.layout_row(node, &children[..children.len().min(1)], ctx),
            Mtable => self.layout_table(node, style),
            Mprescripts | NoneElement => LBox::empty(0.0, 0.0, 0.0),
            // mrow-like elements, and invalid markup (wrong number of children)
            _ => self.layout_row(node, &children, ctx),
        }
    }

    /// Converts the box tree into [`NodeLayout`]s and reports them to the host.
    ///
    /// `frame` is the absolute position of the parent's origin. `owner_origin` is the
    /// absolute border-box origin of the nearest enclosing node, which also receives the
    /// fragments painted by node-less boxes.
    fn emit(
        &mut self,
        lbox: Box_<T>,
        frame: Point,
        owner_origin: Point,
        fragments: &mut Vec<Fragment>,
        suppress_paint: bool,
        return_layout: bool,
    ) -> Option<NodeLayout> {
        let abs = frame + lbox.origin;
        let suppress_paint = suppress_paint || lbox.phantom;
        match lbox.node {
            Some(node) => {
                let m = lbox.margin;
                let metrics = lbox.metrics;
                let border_origin = Point::new(abs.x + m.left, abs.y - metrics.ascent + m.top);
                let mut own_fragments = Vec::new();
                self.emit_content(
                    lbox.content,
                    abs,
                    border_origin,
                    &mut own_fragments,
                    suppress_paint,
                );
                let layout = NodeLayout {
                    location: border_origin - owner_origin,
                    size: Size::new(
                        metrics.advance - m.horizontal(),
                        metrics.height() - m.vertical(),
                    ),
                    baseline: metrics.ascent - m.top,
                    fragments: own_fragments,
                };
                if return_layout {
                    self.tree.set_layout(node, layout.clone());
                    Some(layout)
                } else {
                    self.tree.set_layout(node, layout);
                    None
                }
            }
            None => {
                self.emit_content(lbox.content, abs, owner_origin, fragments, suppress_paint);
                None
            }
        }
    }

    fn emit_content(
        &mut self,
        content: Content<T::NodeId>,
        abs: Point,
        owner_origin: Point,
        fragments: &mut Vec<Fragment>,
        suppress_paint: bool,
    ) {
        match content {
            Content::Empty => {}
            Content::Boxes(children) => {
                for child in children {
                    self.emit(child, abs, owner_origin, fragments, suppress_paint, false);
                }
            }
            Content::Glyphs(run) if !suppress_paint => {
                let scale = run.scale;
                fragments.push(Fragment::Glyphs {
                    origin: abs - owner_origin,
                    font_size: run.font_size,
                    glyphs: run
                        .glyphs
                        .iter()
                        .map(|(pen, glyph)| PositionedGlyph {
                            glyph: glyph.glyph_code,
                            offset: Point::new(
                                (pen.x + glyph.offset.x) as f32 * scale,
                                (pen.y + glyph.offset.y) as f32 * scale,
                            ),
                            cluster: glyph.cluster,
                        })
                        .collect(),
                });
            }
            Content::Rule { width, thickness } if !suppress_paint => {
                let pos = abs - owner_origin;
                fragments.push(Fragment::Rule(Rect::new(
                    pos.x,
                    pos.y - thickness / 2.0,
                    width,
                    thickness,
                )));
            }
            Content::Rect {
                width,
                height,
                dashed,
            } if !suppress_paint => {
                let pos = abs - owner_origin;
                let rect = Rect::new(pos.x, pos.y - height, width, height);
                fragments.push(if dashed {
                    Fragment::DashedRule(rect)
                } else {
                    Fragment::Rule(rect)
                });
            }
            Content::Line { dx, dy, thickness } if !suppress_paint => {
                let start = abs - owner_origin;
                fragments.push(Fragment::Line {
                    start,
                    end: Point::new(start.x + dx, start.y + dy),
                    thickness,
                });
            }
            Content::Glyphs(_)
            | Content::Rule { .. }
            | Content::Rect { .. }
            | Content::Line { .. } => {}
        }
    }
}

/// Applies the CSS box model (MathML Core 3.1.1): adds padding, border and (unless this is the
/// root) margin around a node's content box.
fn inflate<Id: Copy>(content: LBox<Id>, style: &MathComputedStyle, include_margin: bool) -> LBox<Id> {
    let margin = if include_margin {
        style.margin
    } else {
        Edges::default()
    };
    let edges = style.padding + style.border + margin;
    if edges.is_zero() {
        return content;
    }
    let inner_metrics = content.metrics;
    let mut inner = content;
    inner.origin.x += edges.left;
    let advance = inner_metrics.advance + edges.horizontal();
    let horizontal_edges = edges.left != 0.0 || edges.right != 0.0;
    let mut outer = LBox::boxes(vec![inner]);
    outer.metrics = Metrics {
        advance,
        ascent: inner_metrics.ascent + edges.top,
        descent: inner_metrics.descent + edges.bottom,
        ink_left: if horizontal_edges {
            0.0
        } else {
            inner_metrics.ink_left
        },
        ink_width: if horizontal_edges {
            advance
        } else {
            inner_metrics.ink_width
        },
        italic_correction: if edges.right == 0.0 {
            inner_metrics.italic_correction
        } else {
            0.0
        },
        top_accent_attachment: inner_metrics.top_accent_attachment + edges.left,
    };
    outer.margin = margin;
    outer
}
