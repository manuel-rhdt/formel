//! Rows (`<mrow>` and mrow-like elements), tokens, operators and spaces.

use super::lbox::LBox;
use super::operator::OperatorProperties;
use super::{Box_, Ctx, Engine, StretchSize};
use crate::host::{
    Form, MathComputedStyle, MathConstant, MathFont, MathLayoutHost, MathStyle,
    parse_math_length,
};

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
            let (lspace, rspace) = match properties[i] {
                Some(p) if self.applies_operator_spacing(children[i]) => (p.lspace, p.rspace),
                _ => (0.0, 0.0),
            };
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
        let glyphs = font.font.shape(text, Self::shape_options(style, ctx));
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
        let base_glyph = match font.font.shape(text, unflat).first() {
            Some(glyph) => glyph.glyph_code,
            None => return LBox::empty(0.0, 0.0, 0.0),
        };

        if needed_width > 0.0 && font.font.is_stretchable(base_glyph, true) {
            let parts = font.font.stretch(
                base_glyph,
                true,
                font.units(needed_width),
                ctx.as_accent,
                options,
            );
            return LBox::glyphs(font.run(node, parts));
        }

        if needed_height > 0.0 && font.font.is_stretchable(base_glyph, false) {
            let parts = font.font.stretch(
                base_glyph,
                false,
                font.units(needed_height),
                ctx.as_accent,
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

        self.layout_text(node, text, style, ctx)
    }
}
