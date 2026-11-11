//! Rows (`<mrow>` and mrow-like elements), tokens, operators and spaces.

use super::fallback;
use super::lbox::{LBox, ScaledFont};
use super::operator::OperatorProperties;
use super::{Box_, Ctx, Engine, StretchSize};
use crate::host::{
    Form, MathComputedStyle, MathConstant, MathElement, MathFont, MathGlyph, MathLayoutHost,
    MathStyle,
    parse_math_length,
};
use crate::mathmlparser::Flags;
use crate::mathmlparser::operator_dict;

/// The space after a function application, as a fraction of the font size (3/18 em, TeX's
/// thin space).
const THIN_SPACE: f32 = 3.0 / 18.0;

impl<'t, T: MathLayoutHost> Engine<'t, T> {
    /// Lays out `children` of `node` horizontally (MathML Core 3.3.1), with operator spacing and
    /// stretching of stretchy operators to the height of the row.
    pub(crate) fn layout_row(&mut self, node: T::NodeId, children: &[T::NodeId], ctx: Ctx) -> Box_<T> {
        // If this row is an embellished operator, its core child takes the row's place for
        // operator form and stretch size.
        let core_child = self.embellished_row_child(node);

        let in_flow: Vec<usize> = (0..children.len())
            .filter(|&i| !self.is_space_like(children[i]))
            .collect();
        let child_ctxs: Vec<Ctx> = children
            .iter()
            .enumerate()
            .map(|(i, &child)| {
                let form = if Some(child) == core_child {
                    ctx.form
                } else if in_flow.len() > 1 && in_flow.first() == Some(&i) {
                    Form::Prefix
                } else if in_flow.len() > 1 && in_flow.last() == Some(&i) {
                    Form::Postfix
                } else {
                    Form::Infix
                };
                Ctx {
                    stretch: None,
                    form,
                    ..ctx
                }
            })
            .collect();
        let properties: Vec<Option<OperatorProperties>> = children
            .iter()
            .zip(&child_ctxs)
            .map(|(&child, child_ctx)| self.embellished_properties(child, child_ctx.form))
            .collect();

        // Lay out the non-stretchy children first; the stretchy ones then cover their height.
        let is_stretchy = |i: usize| properties[i].is_some_and(|p| p.stretchy);
        let mut boxes: Vec<Option<Box_<T>>> = (0..children.len()).map(|_| None).collect();
        for i in (0..children.len()).filter(|&i| !is_stretchy(i)) {
            boxes[i] = Some(self.layout_node(children[i], child_ctxs[i]));
        }
        let laid_out = || boxes.iter().flatten();
        let row_stretch = StretchSize {
            width: 0.0,
            ascent: laid_out().map(|b| b.metrics.ascent).fold(0.0, f32::max),
            descent: laid_out().map(|b| b.metrics.descent).fold(0.0, f32::max),
        };
        for i in (0..children.len()).filter(|&i| is_stretchy(i)) {
            let stretch = match ctx.stretch {
                Some(outer) if Some(children[i]) == core_child => outer,
                _ => row_stretch,
            };
            let child_ctx = Ctx {
                stretch: Some(stretch),
                ..child_ctxs[i]
            };
            boxes[i] = Some(self.layout_node(children[i], child_ctx));
        }

        let mut cursor = 0.0;
        let mut previous_italic_correction = 0.0;
        let mut row = Vec::with_capacity(children.len());
        for (i, child_box) in boxes.into_iter().enumerate() {
            let mut child_box = child_box.expect("every child was laid out");
            let (lspace, mut rspace) = match properties[i] {
                Some(p) if self.applies_operator_spacing(children[i]) => (p.lspace, p.rspace),
                _ => (0.0, 0.0),
            };
            if self.is_function_application(children[i])
                && self.applies_operator_spacing(children[i])
                && children.get(i + 1).is_some_and(|&next| !self.starts_with_fence(next))
            {
                // `sin x`: a thin space after the function, but none before an opening fence
                // as in `sin(x)`. An explicit `rspace` wins.
                if self.tree.operator_overrides(children[i]).rspace.is_none() {
                    rspace = rspace.max(THIN_SPACE * self.tree.style(children[i]).font_size);
                }
            }
            cursor += lspace;
            // apply italic correction if the current box is upright
            if child_box.metrics.italic_correction == 0.0 {
                cursor += previous_italic_correction;
            }
            child_box.origin.x = cursor;
            cursor += child_box.metrics.advance + rspace;
            previous_italic_correction = if rspace == 0.0 {
                child_box.metrics.italic_correction
            } else {
                0.0
            };
            row.push(child_box);
        }

        let mut row_box = LBox::boxes(row);
        row_box.metrics.advance = row_box.metrics.advance.max(cursor);
        row_box
    }

    /// Whether `node` is the invisible function application operator U+2061.
    fn is_function_application(&self, node: T::NodeId) -> bool {
        self.element(node) == Some(MathElement::Mo)
            && self.tree.text(node).is_some_and(|text| text == "\u{2061}")
    }

    /// Whether the first thing drawn by `node` is an opening fence, looking through rows and
    /// similar wrappers.
    fn starts_with_fence(&self, node: T::NodeId) -> bool {
        use MathElement::*;
        match self.element(node) {
            Some(Mo) => {
                let mut chars = self.tree.text(node).unwrap_or_default().chars().collect::<Vec<_>>();
                match (chars.pop(), chars.is_empty()) {
                    (Some(c), true) => operator_dict::find_entry(c, Form::Prefix).is_some_and(
                        |entry| entry.form == Form::Prefix && entry.flags.contains(Flags::FENCE),
                    ),
                    _ => false,
                }
            }
            Some(Mrow | Mstyle | Mpadded | Mphantom | Semantics) => self
                .tree
                .children(node)
                .find(|&child| !self.is_space_like(child))
                .is_some_and(|first| self.starts_with_fence(first)),
            _ => false,
        }
    }

    /// Operators get their lspace/rspace except in scripts (`math-depth` > 0), as in TeX.
    fn applies_operator_spacing(&self, embellished_op: T::NodeId) -> bool {
        self.tree.style(embellished_op).math_depth <= 0
    }

    pub(crate) fn layout_text(
        &mut self,
        node: T::NodeId,
        text: &str,
        style: &MathComputedStyle,
        ctx: Ctx,
    ) -> Box_<T> {
        let font = self.font(node, style);
        let options = Self::shape_options(style, ctx);
        let mut glyphs = font.font.shape(text, options);
        if glyphs.iter().any(|g| g.glyph_code == 0) {
            if let Some(substituted) = fallback::substitute_missing(&font.font, text, options) {
                glyphs = font.font.shape(&substituted, options);
            }
        }
        give_leading_mark_width(&mut glyphs);
        LBox::glyphs(font.run_from_shaped(node, glyphs))
    }

    pub(crate) fn layout_space(&mut self, node: T::NodeId, style: &MathComputedStyle) -> Box_<T> {
        let length = |name| {
            self.tree
                .attribute(node, name)
                .and_then(|value| parse_math_length(&value, style))
                .unwrap_or(0.0)
        };
        LBox::empty(length("width"), length("height"), length("depth"))
    }

    pub(crate) fn layout_operator(
        &mut self,
        node: T::NodeId,
        style: &MathComputedStyle,
        ctx: Ctx,
    ) -> Box_<T> {
        let properties = self.operator_properties(node, ctx.form);
        let text = self.tree.text(node).unwrap_or_default().into_owned();
        let font = self.font(node, style);
        let text = if ctx.accent_over {
            self.accent_text(&font, &text, style, ctx)
        } else {
            text
        };
        let axis_height = font.constant(MathConstant::AxisHeight);

        match ctx.stretch {
            Some(stretch) if properties.stretchy => {
                let needed_height = if properties.symmetric {
                    (stretch.ascent - axis_height).max(axis_height + stretch.descent) * 2.0
                } else {
                    stretch.ascent + stretch.descent
                };
                let mut needed_height = needed_height;
                if let Some(min) = properties.minsize {
                    needed_height = needed_height.max(min);
                }
                if let Some(max) = properties.maxsize {
                    needed_height = needed_height.min(max);
                }
                self.layout_stretchy(
                    node,
                    &text,
                    style,
                    ctx,
                    needed_height.max(0.0),
                    stretch.width,
                    stretch,
                    properties.symmetric,
                )
            }
            _ if properties.largeop && style.math_style == MathStyle::Normal => {
                // Use the font's DisplayOperatorMinHeight. Only if the font does not provide a
                // usable value, scale the base glyph by sqrt(2).
                let mut min_height = font.constant(MathConstant::DisplayOperatorMinHeight);
                if min_height <= 0.0 {
                    let base = self.layout_text(node, &text, style, Ctx { flat_accent: false, ..ctx });
                    min_height = base.metrics.height() * std::f32::consts::SQRT_2;
                }
                self.layout_stretchy(
                    node,
                    &text,
                    style,
                    ctx,
                    min_height.max(0.0),
                    0.0,
                    StretchSize::default(),
                    true,
                )
            }
            _ => self.layout_text(node, &text, style, ctx),
        }
    }

    /// The text to draw for an accent over a base. Fonts draw ASCII characters and arrows in
    /// their normal position on the line, not as accents above a base, so `^`, `~` and the arrows
    /// are replaced by their combining accents, which have the right shapes and the wide
    /// variants for stretching. Characters the font doesn't have are kept.
    fn accent_text(
        &self,
        font: &ScaledFont<T::Font>,
        text: &str,
        style: &MathComputedStyle,
        ctx: Ctx,
    ) -> String {
        let mut chars = text.chars();
        let replacement = match (chars.next(), chars.next()) {
            (Some('^' | '\u{2C6}'), None) => '\u{302}', // circumflex
            (Some('~' | '\u{2DC}'), None) => '\u{303}', // tilde
            (Some('\u{2192}'), None) => '\u{20D7}',     // right arrow
            (Some('\u{2190}'), None) => '\u{20D6}',     // left arrow
            (Some('\u{2194}'), None) => '\u{20E1}',     // left right arrow
            _ => return text.to_owned(),
        };
        let replacement = replacement.to_string();
        let glyphs = font.font.shape(&replacement, Self::shape_options(style, ctx));
        match glyphs.as_slice() {
            [glyph] if glyph.glyph_code != 0 => replacement,
            _ => text.to_owned(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_stretchy(
        &mut self,
        node: T::NodeId,
        text: &str,
        style: &MathComputedStyle,
        ctx: Ctx,
        needed_height: f32,
        needed_width: f32,
        stretch: StretchSize,
        symmetric: bool,
    ) -> Box_<T> {
        let font = self.font(node, style);
        let unflat = Self::shape_options(style, Ctx { flat_accent: false, ..ctx });
        let options = Self::shape_options(style, ctx);
        let mut text = std::borrow::Cow::Borrowed(text);
        let mut shaped = font.font.shape(&text, unflat);
        if shaped.iter().any(|g| g.glyph_code == 0) {
            if let Some(substituted) = fallback::substitute_missing(&font.font, &text, unflat) {
                shaped = font.font.shape(&substituted, unflat);
                text = substituted.into();
            }
        }
        let Some(&base) = shaped.first() else {
            return LBox::empty(0.0, 0.0, 0.0);
        };
        let base_glyph = base.glyph_code;

        if needed_width > 0.0 {
            let target = font.units(needed_width);
            let too_narrow = base.advance_width.max(base.extents.width) < target;
            let is_bar = fallback::is_bar(&text);
            if font.font.is_stretchable(base_glyph, true) {
                let glyph = base_glyph;
                let parts = font.font.stretch(glyph, true, target, options);
                let stretched = LBox::glyphs(font.run(node, parts));
                // A bar that the font could not stretch far enough is better drawn as a rule.
                if !is_bar || stretched.metrics.ink_width >= 0.9 * needed_width {
                    return stretched;
                }
            }
            if too_narrow && base_glyph != 0 && is_bar {
                // Nothing stretches: draw a rule with the bar glyph's thickness and height.
                let ascent = base.extents.ascent as f32 * font.scale;
                let descent = base.extents.descent as f32 * font.scale;
                return LBox::rule(needed_width, ascent + descent)
                    .shifted_down((descent - ascent) / 2.0);
            }
        }

        if needed_height > 0.0 && font.font.is_stretchable(base_glyph, false) {
            let parts = font.font.stretch(
                base_glyph,
                false,
                font.units(needed_height),
                options,
            );
            let stretched = LBox::glyphs(font.run(node, parts));
            let m = stretched.metrics;
            let shift_down = if symmetric {
                // center on the math axis
                let axis_height = font.constant(MathConstant::AxisHeight);
                -((m.descent - m.ascent) / 2.0 + axis_height)
            } else {
                let excess_ascent = m.ascent - stretch.ascent;
                let excess_descent = m.descent - stretch.descent;
                (excess_ascent - excess_descent) / 2.0
            };
            return stretched.shifted_down(shift_down);
        }

        self.layout_text(node, &text, style, ctx)
    }
}

/// A combining mark with nothing before it to attach to (an accent drawn on its own, or the
/// fallback for a missing spacing accent) has no advance and its ink usually lies left of
/// its origin. Make it a spacing glyph: its ink starts at the origin and the advance covers it.
/// The top accent attachment moves with the ink, so accent placement is unchanged.
fn give_leading_mark_width(glyphs: &mut [MathGlyph]) {
    let Some(first) = glyphs.first_mut() else {
        return;
    };
    if first.advance_width != 0 || first.extents.width <= 0 {
        return;
    }
    let shift = -(first.offset.x + first.extents.left_side_bearing);
    first.offset.x += shift;
    first.top_accent_attachment += shift;
    first.advance_width = first.extents.width;
}
