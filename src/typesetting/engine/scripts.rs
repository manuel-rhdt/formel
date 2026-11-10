//! Scripts (`msub`, `msup`, `msubsup`, `mmultiscripts`) and limits/accents (`munder`,
//! `mover`, `munderover`).

use super::lbox::{LBox, Metrics};
use super::{Box_, Ctx, Engine};
use crate::host::{
    CornerPosition, Form, MathComputedStyle, MathConstant, MathElement, MathLayoutHost,
    MathShift, MathStyle,
};

impl<'t, T: MathLayoutHost> Engine<'t, T> {
    /// The context for the base of a scripted element: the element is an embellished operator
    /// if its base is, so the base inherits form and stretch size.
    fn base_ctx(ctx: Ctx) -> Ctx {
        Ctx {
            stretch: ctx.stretch,
            ..Ctx::plain(ctx.form)
        }
    }

    pub(crate) fn layout_scripts(
        &mut self,
        node: T::NodeId,
        base: T::NodeId,
        subscript: Option<T::NodeId>,
        superscript: Option<T::NodeId>,
        style: &MathComputedStyle,
        ctx: Ctx,
    ) -> Box_<T> {
        let base_is_largeop = self
            .embellished_properties(base, ctx.form)
            .is_some_and(|p| p.largeop);
        let nucleus = self.layout_node(base, Self::base_ctx(ctx));
        let script_ctx = Ctx::plain(Form::Postfix);
        let subscript = subscript.map(|s| self.layout_node(s, script_ctx));
        let superscript = superscript.map(|s| self.layout_node(s, script_ctx));
        self.attach_scripts(node, nucleus, base_is_largeop, subscript, superscript, style)
    }

    /// `<mmultiscripts>`: the base, followed by (subscript, superscript) pairs, optionally
    /// followed by `<mprescripts/>` and more pairs placed before the base.
    pub(crate) fn layout_multiscripts(
        &mut self,
        node: T::NodeId,
        children: &[T::NodeId],
        style: &MathComputedStyle,
        ctx: Ctx,
    ) -> Box_<T> {
        let base = children[0];
        let rest = &children[1..];
        let split = rest
            .iter()
            .position(|&c| self.element(c) == Some(MathElement::Mprescripts))
            .unwrap_or(rest.len());
        let (post, pre) = (&rest[..split], rest.get(split + 1..).unwrap_or(&[]));

        let base_is_largeop = self
            .embellished_properties(base, ctx.form)
            .is_some_and(|p| p.largeop);
        let nucleus = self.layout_node(base, Self::base_ctx(ctx));
        let script_ctx = Ctx::plain(Form::Postfix);
        let pairs = |engine: &mut Self, nodes: &[T::NodeId]| {
            nodes
                .chunks(2)
                .map(|pair| {
                    let mut script = |index: usize| {
                        pair.get(index)
                            .copied()
                            .filter(|&c| engine.element(c) != Some(MathElement::NoneElement))
                            .map(|c| engine.layout_node(c, script_ctx))
                    };
                    (script(0), script(1))
                })
                .collect::<Vec<_>>()
        };
        let post = pairs(self, post);
        let pre = pairs(self, pre);
        self.attach_multiscripts(node, nucleus, base_is_largeop, post, pre, style)
    }

    fn attach_scripts(
        &mut self,
        node: T::NodeId,
        nucleus: Box_<T>,
        nucleus_is_largeop: bool,
        subscript: Option<Box_<T>>,
        superscript: Option<Box_<T>>,
        style: &MathComputedStyle,
    ) -> Box_<T> {
        self.attach_multiscripts(
            node,
            nucleus,
            nucleus_is_largeop,
            vec![(subscript, superscript)],
            Vec::new(),
            style,
        )
    }

    /// Attaches any number of (subscript, superscript) pairs after and before the nucleus.
    /// All scripts share one baseline shift for subscripts and one for superscripts, so that
    /// the indices of a tensor line up.
    fn attach_multiscripts(
        &mut self,
        node: T::NodeId,
        mut nucleus: Box_<T>,
        nucleus_is_largeop: bool,
        post: Vec<(Option<Box_<T>>, Option<Box_<T>>)>,
        pre: Vec<(Option<Box_<T>>, Option<Box_<T>>)>,
        style: &MathComputedStyle,
    ) -> Box_<T> {
        let is_empty = |pairs: &Vec<(Option<Box_<T>>, Option<Box_<T>>)>| {
            pairs.iter().all(|(sub, sup)| sub.is_none() && sup.is_none())
        };
        if is_empty(&post) && is_empty(&pre) {
            return LBox::boxes(vec![nucleus]);
        }
        let font = self.font(node, style);
        let constant = |c| font.constant(c);
        let cramped = style.math_shift == MathShift::Compact;

        let superscript_shift_up = |superscript: &Box_<T>, nucleus: &Box_<T>| {
            let std_shift_up = constant(if cramped {
                MathConstant::SuperscriptShiftUpCramped
            } else {
                MathConstant::SuperscriptShiftUp
            });
            let min_shift_up =
                superscript.metrics.descent + constant(MathConstant::SuperscriptBottomMin);
            let min_shift_from_baseline_drop =
                nucleus.metrics.ascent - constant(MathConstant::SuperscriptBaselineDropMax);
            std_shift_up.max(min_shift_up).max(min_shift_from_baseline_drop)
        };
        let subscript_shift_down = |subscript: &Box_<T>, nucleus: &Box_<T>| {
            let min_shift_from_baseline_drop =
                nucleus.metrics.descent + constant(MathConstant::SubscriptBaselineDropMin);
            let std_shift_down = constant(MathConstant::SubscriptShiftDown);
            let min_shift_down =
                subscript.metrics.ascent - constant(MathConstant::SubscriptTopMax);
            std_shift_down.max(min_shift_down).max(min_shift_from_baseline_drop)
        };

        let pair_shifts = |subscript: &Option<Box_<T>>, superscript: &Option<Box_<T>>| match (subscript, superscript) {
            (Some(sub), Some(sup)) => {
                let mut super_shift = superscript_shift_up(sup, &nucleus);
                let mut sub_shift = subscript_shift_down(sub, &nucleus);

                let gap_min = constant(MathConstant::SubSuperscriptGapMin);
                let super_bottom_max = constant(MathConstant::SuperscriptBottomMaxWithSubscript);
                let super_bottom = super_shift - sup.metrics.descent;
                let sub_top = -sub_shift + sub.metrics.ascent;
                let gap = super_bottom - sub_top;
                if gap < gap_min {
                    let needed_space = gap_min - gap;
                    let super_max_additional_shift = super_bottom_max - super_bottom;
                    if needed_space <= super_max_additional_shift {
                        super_shift += needed_space;
                    } else {
                        super_shift += super_max_additional_shift.max(0.0);
                        sub_shift += needed_space - super_max_additional_shift.max(0.0);
                    }
                }
                (sub_shift, super_shift)
            }
            (Some(sub), None) => (subscript_shift_down(sub, &nucleus), 0.0),
            (None, Some(sup)) => (0.0, superscript_shift_up(sup, &nucleus)),
            (None, None) => (0.0, 0.0),
        };

        let (mut sub_shift, mut super_shift) = (0.0f32, 0.0f32);
        for (sub, sup) in post.iter().chain(&pre) {
            let (down, up) = pair_shifts(sub, sup);
            sub_shift = sub_shift.max(down);
            super_shift = super_shift.max(up);
        }
        let space_after = constant(MathConstant::SpaceAfterScript);

        // Prescripts, right-aligned against the base. The nucleus moves right to make room.
        let mut children = Vec::new();
        let mut cursor = 0.0;
        let pre_count = pre.len();
        for (i, (sub, sup)) in pre.into_iter().enumerate() {
            let width = [&sub, &sup]
                .iter()
                .filter_map(|b| b.as_ref())
                .map(|b| b.metrics.advance)
                .fold(0.0, f32::max);
            for (script, shift) in [(sub, sub_shift), (sup, -super_shift)] {
                if let Some(mut script) = script {
                    script.origin.x = cursor + width - script.metrics.advance;
                    script.origin.y = nucleus.origin.y + shift;
                    children.push(script);
                }
            }
            cursor += width;
            if i + 1 < pre_count {
                cursor += space_after;
            }
        }
        nucleus.origin.x += cursor;

        // Postscripts. Only the first pair gets cut-in kerning against the nucleus.
        let mut post_boxes = Vec::new();
        let mut x = nucleus.origin.x + nucleus.metrics.advance;
        for (i, (sub, sup)) in post.into_iter().enumerate() {
            let mut placed = Vec::with_capacity(2);
            for (script, position, shift) in [
                (sub, CornerPosition::BottomRight, sub_shift),
                (sup, CornerPosition::TopRight, super_shift),
            ] {
                let Some(mut script) = script else { continue };
                if i == 0 {
                    self.position_attachment(
                        &mut script,
                        &nucleus,
                        nucleus_is_largeop,
                        position,
                        shift,
                    );
                } else {
                    script.origin.x = x;
                    script.origin.y = nucleus.origin.y + if position.is_top() { -shift } else { shift };
                }
                placed.push(script);
            }
            let right_edge = placed
                .iter()
                .map(|b| b.origin.x + b.metrics.advance)
                .fold(x, f32::max);
            let mut space = LBox::empty(space_after, 0.0, 0.0);
            space.origin.x = right_edge;
            x = right_edge + space_after;
            post_boxes.extend(placed);
            post_boxes.push(space);
        }

        let mut result = vec![nucleus];
        result.extend(children);
        result.extend(post_boxes);
        LBox::boxes(result)
    }

    fn position_attachment(
        &self,
        attachment: &mut Box_<T>,
        nucleus: &Box_<T>,
        nucleus_is_largeop: bool,
        position: CornerPosition,
        shift: f32,
    ) {
        let kern = self.attachment_kern(nucleus, attachment, position, shift);
        let italic_correction = match (nucleus_is_largeop, position.is_top()) {
            (true, false) => -nucleus.metrics.italic_correction,
            (false, true) => nucleus.metrics.italic_correction,
            _ => 0.0,
        };
        attachment.origin.x =
            nucleus.origin.x + nucleus.metrics.advance + italic_correction + kern;
        attachment.origin.y = nucleus.origin.y + if position.is_top() { -shift } else { shift };
    }

    /// OpenType MATH cut-in kerning between the nucleus and a script.
    fn attachment_kern(
        &self,
        nucleus: &Box_<T>,
        attachment: &Box_<T>,
        position: CornerPosition,
        shift: f32,
    ) -> f32 {
        let (nucleus_glyph, attachment_glyph) = if position.is_left() {
            (nucleus.first_glyph(), attachment.last_glyph())
        } else {
            (nucleus.last_glyph(), attachment.first_glyph())
        };
        let (Some((nucleus_glyph, nucleus_owner, _)), Some((attachment_glyph, attachment_owner, _))) =
            (nucleus_glyph, attachment_glyph)
        else {
            return 0.0;
        };
        let (base_height, attachment_height) = if position.is_top() {
            (
                shift - attachment.metrics.descent,
                nucleus.metrics.ascent - shift,
            )
        } else {
            (
                -shift + attachment.metrics.ascent,
                shift - nucleus.metrics.descent,
            )
        };
        let nucleus_font = self.font(nucleus_owner, &self.tree.style(nucleus_owner));
        let attachment_font = self.font(attachment_owner, &self.tree.style(attachment_owner));
        nucleus_font.math_kerning(nucleus_glyph.glyph_code, position, base_height)
            + attachment_font.math_kerning(
                attachment_glyph.glyph_code,
                position.diagonal_mirror(),
                attachment_height,
            )
    }

    /// Whether the script of an under/over element is an accent. As in MathML Core, only the
    /// `accent`/`accentunder` attribute decides; the operator dictionary is not consulted.
    fn is_accent(&self, node: T::NodeId, attribute: &str, script: Option<T::NodeId>) -> bool {
        script.is_some()
            && self
                .tree
                .attribute(node, attribute)
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
    }

    pub(crate) fn layout_under_over(
        &mut self,
        node: T::NodeId,
        base: T::NodeId,
        under: Option<T::NodeId>,
        over: Option<T::NodeId>,
        style: &MathComputedStyle,
        ctx: Ctx,
    ) -> Box_<T> {
        let base_properties = self.embellished_properties(base, ctx.form);

        // Movable limits become scripts in compact (inline) style.
        if base_properties.is_some_and(|p| p.movablelimits)
            && style.math_style == MathStyle::Compact
        {
            return self.layout_scripts(node, base, under, over, style, ctx);
        }

        let base_is_largeop = base_properties.is_some_and(|p| p.largeop);
        let base_is_stretchy = base_properties.is_some_and(|p| p.stretchy);
        let over_is_accent = self.is_accent(node, "accent", over);
        let under_is_accent = self.is_accent(node, "accentunder", under);

        let script_ctx = Ctx::plain(Form::Postfix);
        // (node, context, is accent)
        let slots = [
            (Some(base), Self::base_ctx(ctx), false),
            (over, script_ctx, over_is_accent),
            (under, script_ctx, under_is_accent),
        ];
        let is_stretchy = |engine: &Self, slot: &(Option<T::NodeId>, Ctx, bool)| {
            slot.0.is_some_and(|n| {
                engine
                    .embellished_properties(n, slot.1.form)
                    .is_some_and(|p| p.stretchy)
            })
        };

        // First the non-stretchy parts, then stretch the others to their widest.
        let mut boxes: [Option<Box_<T>>; 3] = [None, None, None];
        for (i, slot) in slots.iter().enumerate() {
            if !is_stretchy(self, slot) {
                boxes[i] = slot.0.map(|n| self.layout_node(n, slot.1));
            }
        }
        let mut max_width = boxes
            .iter()
            .flatten()
            .map(|b| b.metrics.ink_width)
            .fold(0.0, f32::max);
        if let Some(stretch) = ctx.stretch {
            max_width = max_width.max(stretch.width);
        }
        let mut stretch_ctxs = [slots[0].1, slots[1].1, slots[2].1];
        for (i, slot) in slots.iter().enumerate() {
            let mut stretch = slot.1.stretch.unwrap_or_default();
            stretch.width = max_width;
            stretch_ctxs[i] = Ctx {
                stretch: Some(stretch),
                as_accent: slot.2,
                ..slot.1
            };
            if is_stretchy(self, slot) {
                boxes[i] = slot.0.map(|n| self.layout_node(n, stretch_ctxs[i]));
            }
        }

        let [nucleus, over_box, under_box] = boxes;
        let mut nucleus = nucleus.expect("the base is always laid out");
        let font = self.font(node, style);

        if let Some(mut over_box) = over_box {
            // use flattened accents over tall bases
            if over_is_accent
                && nucleus.metrics.ascent >= font.constant(MathConstant::FlattenedAccentBaseHeight)
            {
                let flat_ctx = Ctx {
                    flat_accent: true,
                    ..stretch_ctxs[1]
                };
                over_box = self.layout_node(over.expect("over box exists"), flat_ctx);
            }
            nucleus = self.attach_under_over(
                node,
                style,
                over_box,
                nucleus,
                true,
                over_is_accent,
                base_is_largeop,
                base_is_stretchy,
            );
        }
        if let Some(under_box) = under_box {
            nucleus = self.attach_under_over(
                node,
                style,
                under_box,
                nucleus,
                false,
                under_is_accent,
                base_is_largeop,
                base_is_stretchy,
            );
        }
        nucleus
    }

    #[allow(clippy::too_many_arguments)]
    fn attach_under_over(
        &self,
        node: T::NodeId,
        style: &MathComputedStyle,
        mut attachment: Box_<T>,
        mut nucleus: Box_<T>,
        as_over: bool,
        as_accent: bool,
        nucleus_is_largeop: bool,
        nucleus_is_stretchy: bool,
    ) -> Box_<T> {
        let font = self.font(node, style);
        let constant = |c| font.constant(c);
        let mut gap = 0.0;
        let mut shift = 0.0;
        if nucleus_is_largeop {
            if as_over {
                gap = constant(MathConstant::UpperLimitGapMin);
                shift = constant(MathConstant::UpperLimitBaselineRiseMin) + nucleus.metrics.ascent;
            } else {
                gap = constant(MathConstant::LowerLimitGapMin);
                shift =
                    constant(MathConstant::LowerLimitBaselineDropMin) + nucleus.metrics.descent;
            }
        } else if nucleus_is_stretchy {
            if as_over {
                gap = constant(MathConstant::StretchStackGapBelowMin);
                shift = constant(MathConstant::StretchStackTopShiftUp);
            } else {
                gap = constant(MathConstant::StretchStackGapAboveMin);
                shift = constant(MathConstant::StretchStackBottomShiftDown);
            }
        } else if !as_accent {
            gap = if as_over {
                constant(MathConstant::OverbarVerticalGap)
            } else {
                constant(MathConstant::UnderbarVerticalGap)
            };
            shift = gap;
        }

        let baseline_offset = if as_accent {
            if as_over {
                let accent_base_height = constant(MathConstant::AccentBaseHeight);
                -(nucleus.metrics.ascent - accent_base_height).max(0.0)
            } else {
                nucleus.metrics.descent
            }
        } else if as_over {
            -shift.max(nucleus.metrics.ascent + gap + attachment.metrics.descent)
        } else {
            shift.max(nucleus.metrics.descent + gap + attachment.metrics.ascent)
        };
        attachment.origin.y = nucleus.origin.y + baseline_offset;

        // centering
        let center_difference = if as_accent && as_over {
            (nucleus.origin.x + nucleus.metrics.top_accent_attachment)
                - (attachment.origin.x + attachment.metrics.top_accent_attachment)
        } else {
            (nucleus.origin.x + nucleus.metrics.ink_center())
                - (attachment.origin.x + attachment.metrics.ink_center())
        };
        if center_difference < 0.0 && !as_accent {
            nucleus.origin.x -= center_difference;
        } else {
            attachment.origin.x += center_difference;
        }

        // italic correction of large operators: limits follow the slant
        if nucleus_is_largeop {
            let half_italic = nucleus.metrics.italic_correction / 2.0;
            attachment.origin.x += if as_over { half_italic } else { -half_italic };
        }

        let advance = if as_accent {
            nucleus.metrics.advance
        } else {
            (nucleus.origin.x + nucleus.metrics.advance)
                .max(attachment.origin.x + attachment.metrics.advance)
        };
        let italic_correction = if as_accent {
            nucleus.metrics.italic_correction
        } else {
            0.0
        };
        let top_accent_attachment = if as_over {
            attachment.origin.x + attachment.metrics.top_accent_attachment
        } else {
            nucleus.origin.x + nucleus.metrics.top_accent_attachment
        };

        let mut result = LBox::boxes(vec![nucleus, attachment]);
        result.metrics = Metrics {
            advance,
            italic_correction,
            top_accent_attachment,
            ..result.metrics
        };
        result
    }
}
