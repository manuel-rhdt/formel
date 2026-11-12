//! Fractions and radicals.

use super::lbox::LBox;
use super::{Box_, Ctx, Engine};
use crate::host::{
    Form, MathComputedStyle, MathConstant, MathFont, MathLayoutHost, MathStyle, Point,
    parse_math_length, script_scale_factor,
};

/// TeX's `\nulldelimiterspace`, 1.2pt at 10pt, as a fraction of the font size.
const NULL_DELIMITER_SPACE: f32 = 0.12;

impl<'t, T: MathLayoutHost> Engine<'t, T> {
    /// `<mfrac>` (MathML Core 3.3.2).
    pub(crate) fn layout_fraction(
        &mut self,
        node: T::NodeId,
        numerator: T::NodeId,
        denominator: T::NodeId,
        style: &MathComputedStyle,
        ctx: Ctx,
    ) -> Box_<T> {
        // An mfrac whose numerator is an embellished operator is itself one.
        let numerator_ctx = Ctx {
            stretch: ctx.stretch,
            ..Ctx::plain(ctx.form)
        };
        let mut numerator = self.layout_node(numerator, numerator_ctx);
        let mut denominator = self.layout_node(denominator, Ctx::plain(Form::Infix));

        let font = self.font(node, style);
        let display = style.math_style == MathStyle::Normal;
        let default_thickness = font.constant(MathConstant::FractionRuleThickness);
        let thickness = self.fraction_thickness(node, style, default_thickness);
        let axis_height = font.constant(MathConstant::AxisHeight);

        let mut children = Vec::with_capacity(3);
        if thickness > 0.0 {
            let (shift_up, shift_down) = if display {
                (
                    font.constant(MathConstant::FractionNumeratorDisplayStyleShiftUp),
                    font.constant(MathConstant::FractionDenominatorDisplayStyleShiftDown),
                )
            } else {
                (
                    font.constant(MathConstant::FractionNumeratorShiftUp),
                    font.constant(MathConstant::FractionDenominatorShiftDown),
                )
            };
            let (numerator_gap_min, denominator_gap_min) = if display {
                (
                    font.constant(MathConstant::FractionNumDisplayStyleGapMin),
                    font.constant(MathConstant::FractionDenomDisplayStyleGapMin),
                )
            } else {
                (
                    font.constant(MathConstant::FractionNumeratorGapMin),
                    font.constant(MathConstant::FractionDenominatorGapMin),
                )
            };

            // shifts relative to the math axis
            let numerator_shift_up = (shift_up - axis_height)
                .max(numerator_gap_min + thickness / 2.0 + numerator.metrics.descent);
            let denominator_shift_down = (shift_down + axis_height)
                .max(denominator_gap_min + thickness / 2.0 + denominator.metrics.ascent);
            numerator.origin.y = -axis_height - numerator_shift_up;
            denominator.origin.y = -axis_height + denominator_shift_down;
        } else {
            // A stack without a fraction bar.
            let (mut shift_up, mut shift_down, gap_min) = if display {
                (
                    font.constant(MathConstant::StackTopDisplayStyleShiftUp),
                    font.constant(MathConstant::StackBottomDisplayStyleShiftDown),
                    font.constant(MathConstant::StackDisplayStyleGapMin),
                )
            } else {
                (
                    font.constant(MathConstant::StackTopShiftUp),
                    font.constant(MathConstant::StackBottomShiftDown),
                    font.constant(MathConstant::StackGapMin),
                )
            };
            let gap = (shift_up - numerator.metrics.descent)
                - (denominator.metrics.ascent - shift_down);
            if gap < gap_min {
                let delta = (gap_min - gap) / 2.0;
                shift_up += delta;
                shift_down += delta;
            }
            numerator.origin.y = -shift_up;
            denominator.origin.y = shift_down;
        }

        // Centre the numerator and the denominator by their margin boxes (MathML Core 3.3.2, and TeX
        // does the same), not by their ink: the fraction is as wide as the wider one, and the bar
        // spans all of it.
        let width = numerator.metrics.advance.max(denominator.metrics.advance);
        // TeX sets a fraction between two empty delimiters, each `\nulldelimiterspace` wide.
        // It is a fixed length: it does not shrink in a script (unlike the font size).
        let text_size = style.font_size
            / script_scale_factor(&self.tree.font(node), 0, style.math_depth);
        let margin = NULL_DELIMITER_SPACE * text_size;
        numerator.origin.x = margin + (width - numerator.metrics.advance) / 2.0;
        denominator.origin.x = margin + (width - denominator.metrics.advance) / 2.0;

        if thickness > 0.0 {
            let mut rule = LBox::rule(width, thickness);
            rule.origin = Point::new(margin, -axis_height);
            children.push(numerator);
            children.push(rule);
            children.push(denominator);
        } else {
            children.push(numerator);
            children.push(denominator);
        }

        let mut fraction = LBox::boxes(children);
        fraction.metrics.italic_correction = 0.0;
        fraction.metrics.advance = margin + width + margin;
        fraction
    }

    /// The `linethickness` attribute: a length, a percentage of the default thickness, or
    /// (legacy) a multiple of it.
    fn fraction_thickness(
        &self,
        node: T::NodeId,
        style: &MathComputedStyle,
        default_thickness: f32,
    ) -> f32 {
        let Some(value) = self.tree.attribute(node, "linethickness") else {
            return default_thickness;
        };
        let value = value.trim();
        if let Some(percent) = value.strip_suffix('%') {
            return percent
                .trim()
                .parse::<f32>()
                .map_or(default_thickness, |p| default_thickness * p / 100.0);
        }
        match value {
            "thin" => default_thickness / 2.0,
            "medium" => default_thickness,
            "thick" => default_thickness * 2.0,
            _ => parse_math_length(value, style)
                .or_else(|| value.parse::<f32>().ok().map(|n| n * default_thickness))
                .unwrap_or(default_thickness),
        }
        .max(0.0)
    }

    /// `<msqrt>` (`index` is `None`) and `<mroot>` (MathML Core 3.3.3).
    pub(crate) fn layout_radical(
        &mut self,
        node: T::NodeId,
        index: Option<T::NodeId>,
        base_children: &[T::NodeId],
        style: &MathComputedStyle,
    ) -> Box_<T> {
        // msqrt has an inferred mrow; mroot's base is a single node.
        let mut radicand = match (index, base_children) {
            (Some(_), [base]) => self.layout_node(*base, Ctx::plain(Form::Infix)),
            _ => self.layout_row(node, base_children, Ctx::plain(Form::Infix)),
        };

        let font = self.font(node, style);
        let options = Self::shape_options(style, Ctx::default());
        let thickness = font.constant(MathConstant::RadicalRuleThickness);
        let vertical_gap = if style.math_style == MathStyle::Normal {
            font.constant(MathConstant::RadicalDisplayStyleVerticalGap)
        } else {
            font.constant(MathConstant::RadicalVerticalGap)
        };
        let extra_ascender = font.constant(MathConstant::RadicalExtraAscender);

        // stretch the surd to cover the radicand
        let needed_surd_height = radicand.metrics.height() + vertical_gap + thickness;
        let surd_glyphs = font.font.shape("\u{221A}", options);
        let mut surd = match surd_glyphs.first() {
            Some(glyph) if font.font.is_stretchable(glyph.glyph_code, false) => {
                let parts = font.font.stretch(
                    glyph.glyph_code,
                    false,
                    font.units(needed_surd_height),
                    options,
                );
                LBox::glyphs(font.run(node, parts))
            }
            _ => LBox::glyphs(font.run_from_shaped(node, surd_glyphs)),
        };

        // Center the radicand vertically in the space the surd leaves.
        let surd_excess_height = surd.metrics.height() - needed_surd_height;
        surd.origin.y =
            (radicand.metrics.descent - surd.metrics.descent) + surd_excess_height / 2.0;

        radicand.origin.x = surd.origin.x + surd.metrics.advance;

        let mut rule = LBox::rule(radicand.metrics.advance, thickness);
        rule.origin = Point::new(
            surd.origin.x + surd.metrics.advance,
            surd.origin.y - surd.metrics.ascent + thickness / 2.0,
        );
        let rule_top = rule.origin.y - thickness / 2.0;

        let mut children = Vec::with_capacity(4);
        if let Some(index) = index {
            let raise_percent =
                font.font.constant(MathConstant::RadicalDegreeBottomRaisePercent) as f32 / 100.0;
            let kern_before = font.constant(MathConstant::RadicalKernBeforeDegree);
            let kern_after = font.constant(MathConstant::RadicalKernAfterDegree);
            let degree_bottom = surd.origin.y + surd.metrics.descent
                - surd.metrics.height() * raise_percent;

            let mut degree = self.layout_node(index, Ctx::plain(Form::Infix));
            // `degree_bottom` is where the bottom of the degree goes, not its baseline: the font's
            // `RadicalDegreeBottomRaisePercent` is the "height of the bottom of the radical degree",
            // and so is MathML Core (3.3.3), which adds the descent of the degree's box. TeX raises
            // the baseline of the degree instead, so an index with a descender (`p`, `g`) is about
            // 100 units lower in TeX than here; see `KNOWN_GLYPH_DIFFERENCES` of `tex_oracle`.
            degree.origin.y += degree_bottom - degree.metrics.descent;
            degree.origin.x += kern_before;

            let surd_kern = (kern_before + degree.metrics.advance + kern_after).max(0.0);
            surd.origin.x += surd_kern;
            radicand.origin.x += surd_kern;
            rule.origin.x += surd_kern;
            children.push(degree);
        }

        children.push(surd);
        children.push(rule);
        children.push(radicand);
        let mut radical = LBox::boxes(children);
        radical.metrics.ascent = radical.metrics.ascent.max(extra_ascender - rule_top);
        radical
    }
}
