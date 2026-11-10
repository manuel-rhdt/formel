//! A [`MathFont`] implementation based on [rustybuzz](https://docs.rs/rustybuzz) for shaping and
//! [ttf-parser](https://docs.rs/ttf-parser) for reading the OpenType `MATH` table.

use std::cell::RefCell;
use std::cmp::min;

use rustybuzz::ttf_parser::gsub::SubstitutionSubtable;
use rustybuzz::ttf_parser::math::{self, GlyphConstruction};
use rustybuzz::ttf_parser::{GlyphId, Tag};
use rustybuzz::{Direction, Face, Feature, GlyphBuffer, Script, UnicodeBuffer};

use crate::host::{
    CornerPosition, Extents, GlyphCode, MathConstant, MathFont, MathGlyph, ShapeOptions, Vector,
};

/// A font with a `MATH` table, shaped with rustybuzz.
pub struct Shaper<'a> {
    face: Face<'a>,
    math: math::Table<'a>,
    buffer: RefCell<Option<UnicodeBuffer>>,
}

impl std::fmt::Debug for Shaper<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shaper").finish_non_exhaustive()
    }
}

impl<'a> Shaper<'a> {
    /// Creates a shaper for `face`. Returns `None` if the font has no `MATH` table.
    pub fn new(face: Face<'a>) -> Option<Shaper<'a>> {
        let math = face.tables().math?;
        Some(Shaper {
            face,
            math,
            buffer: RefCell::new(Some(UnicodeBuffer::new())),
        })
    }

    /// Parses font data and creates a shaper for it. Returns `None` if the data cannot be
    /// parsed or the font has no `MATH` table.
    pub fn from_slice(data: &'a [u8], face_index: u32) -> Option<Shaper<'a>> {
        Self::new(Face::from_slice(data, face_index)?)
    }

    /// The underlying font face, e.g. to draw glyph outlines.
    pub fn face(&self) -> &Face<'a> {
        &self.face
    }

    /// The value of a MATH table constant.
    pub fn math_constant(&self, c: MathConstant) -> i32 {
        use MathConstant::*;
        let Some(k) = self.math.constants else {
            return 0;
        };
        // Cambria Math has incorrect values for these two constants. Like HarfBuzz (and,
        // apparently, Microsoft's implementation) we swap them when we detect that font.
        // See https://github.com/harfbuzz/harfbuzz/issues/4653
        let c = match c {
            DisplayOperatorMinHeight if self.is_bad_cambria() => DelimitedSubFormulaMinHeight,
            DelimitedSubFormulaMinHeight if self.is_bad_cambria() => DisplayOperatorMinHeight,
            c => c,
        };
        let v = |m: math::MathValue| m.value as i32;
        match c {
            ScriptPercentScaleDown => k.script_percent_scale_down() as i32,
            ScriptScriptPercentScaleDown => k.script_script_percent_scale_down() as i32,
            DelimitedSubFormulaMinHeight => k.delimited_sub_formula_min_height() as i32,
            DisplayOperatorMinHeight => k.display_operator_min_height() as i32,
            MathLeading => v(k.math_leading()),
            AxisHeight => v(k.axis_height()),
            AccentBaseHeight => v(k.accent_base_height()),
            FlattenedAccentBaseHeight => v(k.flattened_accent_base_height()),
            SubscriptShiftDown => v(k.subscript_shift_down()),
            SubscriptTopMax => v(k.subscript_top_max()),
            SubscriptBaselineDropMin => v(k.subscript_baseline_drop_min()),
            SuperscriptShiftUp => v(k.superscript_shift_up()),
            SuperscriptShiftUpCramped => v(k.superscript_shift_up_cramped()),
            SuperscriptBottomMin => v(k.superscript_bottom_min()),
            SuperscriptBaselineDropMax => v(k.superscript_baseline_drop_max()),
            SubSuperscriptGapMin => v(k.sub_superscript_gap_min()),
            SuperscriptBottomMaxWithSubscript => v(k.superscript_bottom_max_with_subscript()),
            SpaceAfterScript => v(k.space_after_script()),
            UpperLimitGapMin => v(k.upper_limit_gap_min()),
            UpperLimitBaselineRiseMin => v(k.upper_limit_baseline_rise_min()),
            LowerLimitGapMin => v(k.lower_limit_gap_min()),
            LowerLimitBaselineDropMin => v(k.lower_limit_baseline_drop_min()),
            StackTopShiftUp => v(k.stack_top_shift_up()),
            StackTopDisplayStyleShiftUp => v(k.stack_top_display_style_shift_up()),
            StackBottomShiftDown => v(k.stack_bottom_shift_down()),
            StackBottomDisplayStyleShiftDown => v(k.stack_bottom_display_style_shift_down()),
            StackGapMin => v(k.stack_gap_min()),
            StackDisplayStyleGapMin => v(k.stack_display_style_gap_min()),
            StretchStackTopShiftUp => v(k.stretch_stack_top_shift_up()),
            StretchStackBottomShiftDown => v(k.stretch_stack_bottom_shift_down()),
            StretchStackGapAboveMin => v(k.stretch_stack_gap_above_min()),
            StretchStackGapBelowMin => v(k.stretch_stack_gap_below_min()),
            FractionNumeratorShiftUp => v(k.fraction_numerator_shift_up()),
            FractionNumeratorDisplayStyleShiftUp => {
                v(k.fraction_numerator_display_style_shift_up())
            }
            FractionDenominatorShiftDown => v(k.fraction_denominator_shift_down()),
            FractionDenominatorDisplayStyleShiftDown => {
                v(k.fraction_denominator_display_style_shift_down())
            }
            FractionNumeratorGapMin => v(k.fraction_numerator_gap_min()),
            FractionNumDisplayStyleGapMin => v(k.fraction_num_display_style_gap_min()),
            FractionRuleThickness => v(k.fraction_rule_thickness()),
            FractionDenominatorGapMin => v(k.fraction_denominator_gap_min()),
            FractionDenomDisplayStyleGapMin => v(k.fraction_denom_display_style_gap_min()),
            SkewedFractionHorizontalGap => v(k.skewed_fraction_horizontal_gap()),
            SkewedFractionVerticalGap => v(k.skewed_fraction_vertical_gap()),
            OverbarVerticalGap => v(k.overbar_vertical_gap()),
            OverbarRuleThickness => v(k.overbar_rule_thickness()),
            OverbarExtraAscender => v(k.overbar_extra_ascender()),
            UnderbarVerticalGap => v(k.underbar_vertical_gap()),
            UnderbarRuleThickness => v(k.underbar_rule_thickness()),
            UnderbarExtraDescender => v(k.underbar_extra_descender()),
            RadicalVerticalGap => v(k.radical_vertical_gap()),
            RadicalDisplayStyleVerticalGap => v(k.radical_display_style_vertical_gap()),
            RadicalRuleThickness => v(k.radical_rule_thickness()),
            RadicalExtraAscender => v(k.radical_extra_ascender()),
            RadicalKernBeforeDegree => v(k.radical_kern_before_degree()),
            RadicalKernAfterDegree => v(k.radical_kern_after_degree()),
            RadicalDegreeBottomRaisePercent => k.radical_degree_bottom_raise_percent() as i32,
        }
    }

    /// Detects the known-bad MATH table of Cambria Math, which has `displayOperatorMinHeight`
    /// and `delimitedSubFormulaMinHeight` swapped. The check is identical to HarfBuzz'
    /// `is_bad_cambria`.
    fn is_bad_cambria(&self) -> bool {
        let Some(k) = self.math.constants else {
            return false;
        };
        self.math_table().len() == 25722
            && k.display_operator_min_height() == 2500
            && k.delimited_sub_formula_min_height() == 3000
    }

    /// The raw OpenType MATH table.
    pub fn math_table(&self) -> &[u8] {
        self.face
            .raw_face()
            .table(Tag::from_bytes(b"MATH"))
            .expect("MATH table must be present")
    }

    fn features(options: ShapeOptions) -> Vec<Feature> {
        let mut features = Vec::with_capacity(2);
        if options.script_level >= 1 {
            features.push(Feature::new(
                Tag::from_bytes(b"ssty"),
                options.script_level as u32,
                ..,
            ));
        }
        if options.flat_accent {
            features.push(Feature::new(Tag::from_bytes(b"flac"), 1, ..));
        }
        features
    }

    fn shape_str(&self, string: &str, options: ShapeOptions) -> Vec<MathGlyph> {
        let mut buffer = self.buffer.borrow_mut().take().unwrap_or_default();
        buffer.push_str(string);
        // The ISO 15924 tag `Math` selects the OpenType `math` script (rustybuzz' own
        // `SCRIPT_MATH`, `Zmth`, would map to `zmth` instead).
        if let Some(math_script) = Script::from_iso15924_tag(Tag::from_bytes(b"Math")) {
            buffer.set_script(math_script);
        }
        buffer.set_direction(Direction::LeftToRight);

        let glyph_buffer = rustybuzz::shape(&self.face, &Self::features(options), buffer);
        let glyphs = self.layout_boxes(&glyph_buffer);
        *self.buffer.borrow_mut() = Some(glyph_buffer.clear());
        glyphs
    }

    fn layout_boxes(&self, glyph_buffer: &GlyphBuffer) -> Vec<MathGlyph> {
        let positions = glyph_buffer.glyph_positions();
        let infos = glyph_buffer.glyph_infos();
        positions
            .iter()
            .zip(infos.iter())
            .map(|(pos, info)| {
                self.math_glyph(
                    GlyphId(info.glyph_id as u16),
                    info.cluster,
                    pos.x_advance,
                    Vector {
                        x: pos.x_offset,
                        // harfbuzz' y axis points upwards
                        y: -pos.y_offset,
                    },
                )
            })
            .collect()
    }

    /// The glyph with the given glyph index (bypassing the cmap), after applying the GSUB
    /// features requested by `options`.
    fn glyphs_from_index(&self, glyph_index: u32, options: ShapeOptions) -> Vec<MathGlyph> {
        let glyph = self.substitute(GlyphId(glyph_index as u16), options);
        let is_mark = self
            .face
            .tables()
            .gdef
            .and_then(|gdef| gdef.glyph_class(glyph))
            == Some(rustybuzz::ttf_parser::gdef::GlyphClass::Mark);
        let advance = if is_mark {
            0
        } else {
            self.face.glyph_hor_advance(glyph).unwrap_or(0) as i32
        };
        vec![self.math_glyph(glyph, 0, advance, Vector { x: 0, y: 0 })]
    }

    /// Applies the single and alternate substitutions of the features in `options` to one glyph.
    fn substitute(&self, mut glyph: GlyphId, options: ShapeOptions) -> GlyphId {
        let Some(gsub) = self.face.tables().gsub else {
            return glyph;
        };
        let mut wanted = Vec::with_capacity(2);
        if options.script_level >= 1 {
            wanted.push((Tag::from_bytes(b"ssty"), options.script_level as u16));
        }
        if options.flat_accent {
            wanted.push((Tag::from_bytes(b"flac"), 1));
        }
        if wanted.is_empty() {
            return glyph;
        }

        let script = gsub
            .scripts
            .find(Tag::from_bytes(b"math"))
            .or_else(|| gsub.scripts.find(Tag::from_bytes(b"DFLT")));
        let Some(language) = script.and_then(|s| s.default_language) else {
            return glyph;
        };

        // (lookup index, feature value)
        let mut lookups = Vec::new();
        for feature_index in language.feature_indices {
            let Some(feature) = gsub.features.get(feature_index) else {
                continue;
            };
            if let Some(&(_, value)) = wanted.iter().find(|(tag, _)| *tag == feature.tag) {
                lookups.extend(feature.lookup_indices.into_iter().map(|i| (i, value)));
            }
        }
        lookups.sort_by_key(|&(index, _)| index);
        lookups.dedup();

        for (lookup_index, value) in lookups {
            let Some(lookup) = gsub.lookups.get(lookup_index) else {
                continue;
            };
            for subtable in lookup.subtables.into_iter::<SubstitutionSubtable>() {
                let replacement = match subtable {
                    SubstitutionSubtable::Single(single) => match single {
                        rustybuzz::ttf_parser::gsub::SingleSubstitution::Format1 {
                            coverage,
                            delta,
                        } => coverage
                            .contains(glyph)
                            .then(|| GlyphId((glyph.0 as i32 + delta as i32) as u16)),
                        rustybuzz::ttf_parser::gsub::SingleSubstitution::Format2 {
                            coverage,
                            substitutes,
                        } => coverage.get(glyph).and_then(|i| substitutes.get(i)),
                    },
                    SubstitutionSubtable::Alternate(alt) => alt
                        .coverage
                        .get(glyph)
                        .and_then(|i| alt.alternate_sets.get(i))
                        .and_then(|set| set.alternates.get(value.checked_sub(1)?)),
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    glyph = replacement;
                    break;
                }
            }
        }
        glyph
    }

    fn math_glyph(
        &self,
        glyph: GlyphId,
        cluster: u32,
        advance_width: i32,
        offset: Vector<i32>,
    ) -> MathGlyph {
        let extents = match self.face.glyph_bounding_box(glyph) {
            Some(rect) => Extents {
                left_side_bearing: rect.x_min as i32,
                width: rect.x_max as i32 - rect.x_min as i32,
                ascent: rect.y_max as i32,
                descent: -(rect.y_min as i32),
            },
            None => Extents {
                left_side_bearing: 0,
                width: 0,
                ascent: 0,
                descent: 0,
            },
        };
        let glyph_info = self.math.glyph_info;
        MathGlyph {
            glyph_code: glyph.0 as u32,
            cluster,
            offset,
            advance_width,
            extents,
            italic_correction: glyph_info
                .and_then(|info| info.italic_corrections)
                .and_then(|values| values.get(glyph))
                .map_or(0, |v| v.value as i32),
            // Like HarfBuzz: glyphs without an entry are attached at half their advance.
            top_accent_attachment: glyph_info
                .and_then(|info| info.top_accent_attachments)
                .and_then(|values| values.get(glyph))
                .map_or_else(
                    || self.face.glyph_hor_advance(glyph).unwrap_or(0) as i32 / 2,
                    |v| v.value as i32,
                ),
        }
    }

    fn construction(&self, glyph: u32, horizontal: bool) -> Option<GlyphConstruction<'a>> {
        let variants = self.math.variants?;
        let constructions = if horizontal {
            variants.horizontal_constructions
        } else {
            variants.vertical_constructions
        };
        constructions.get(GlyphId(glyph as u16))
    }

    /// The size variants of a glyph in increasing size, as (glyph, advance) pairs.
    fn variants(&self, glyph: u32, horizontal: bool) -> Vec<(u32, i32)> {
        self.construction(glyph, horizontal)
            .map(|c| {
                c.variants
                    .into_iter()
                    .map(|v| (v.variant_glyph.0 as u32, v.advance_measurement as i32))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn assembly_parts(&self, glyph: u32, horizontal: bool) -> Vec<Part> {
        self.construction(glyph, horizontal)
            .and_then(|c| c.assembly)
            .map(|a| {
                a.parts
                    .into_iter()
                    .map(|p| Part {
                        glyph: p.glyph_id.0 as u32,
                        start_connector_length: p.start_connector_length as i32,
                        end_connector_length: p.end_connector_length as i32,
                        full_advance: p.full_advance as i32,
                        extender: p.part_flags.extender(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn stretch_parts(
        &self,
        glyph: u32,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> Stretched {
        // MathML Core 5.3.2: the base glyph if it is big enough, else the first variant that
        // is, else an assembly, else the last thing that was tried (the largest variant).
        try_base_glyph(self, glyph, horizontal, target_size, options)
            .or_else(|| try_variant(self, glyph, horizontal, target_size, options))
            .map(Stretched::Single)
            .or_else(|| {
                try_assembly(self, glyph, horizontal, target_size, options).map(Stretched::Assembly)
            })
            .or_else(|| largest_variant(self, glyph, horizontal, options).map(Stretched::Single))
            .unwrap_or_else(|| Stretched::Single(self.glyphs_from_index(glyph, options)))
    }

    fn has_variants_or_assembly(&self, glyph: u32, horizontal: bool) -> bool {
        self.construction(glyph, horizontal)
            .is_some_and(|c| c.variants.len() > 0 || c.assembly.is_some_and(|a| a.parts.len() > 0))
    }
}

/// A part of a glyph assembly.
#[derive(Debug, Clone, Copy)]
struct Part {
    glyph: u32,
    start_connector_length: i32,
    end_connector_length: i32,
    full_advance: i32,
    extender: bool,
}

/// The result of stretching a glyph: either a single glyph (a variant) or an assembly of
/// parts, each with its offset from the first part.
enum Stretched {
    Single(Vec<MathGlyph>),
    Assembly(Vec<(Vector<i32>, Vec<MathGlyph>)>),
}

fn point_with_offset(offset: i32, horizontal: bool) -> Vector<i32> {
    if horizontal {
        Vector { x: offset, y: 0 }
    } else {
        Vector { x: 0, y: offset }
    }
}

impl<'a> MathFont for Shaper<'a> {
    fn units_per_em(&self) -> u32 {
        self.face.units_per_em() as u32
    }

    fn constant(&self, c: MathConstant) -> i32 {
        self.math_constant(c)
    }

    fn shape(&self, text: &str, options: ShapeOptions) -> Vec<MathGlyph> {
        self.shape_str(text, options)
    }

    fn glyph(&self, glyph: GlyphCode, options: ShapeOptions) -> MathGlyph {
        self.glyphs_from_index(glyph, options)
            .first()
            .copied()
            .unwrap_or_default()
    }

    fn is_stretchable(&self, glyph: GlyphCode, horizontal: bool) -> bool {
        self.has_variants_or_assembly(glyph, horizontal)
    }

    fn stretch(
        &self,
        glyph: GlyphCode,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> Vec<(Vector<i32>, MathGlyph)> {
        match self.stretch_parts(glyph, horizontal, target_size, options) {
            Stretched::Single(glyphs) => glyphs
                .into_iter()
                .scan(0, |advance, glyph| {
                    let offset = Vector { x: *advance, y: 0 };
                    *advance += glyph.advance_width;
                    Some((offset, glyph))
                })
                .collect(),
            Stretched::Assembly(parts) => parts
                .into_iter()
                .flat_map(|(origin, glyphs)| glyphs.into_iter().map(move |g| (origin, g)))
                .collect(),
        }
    }

    fn math_kerning(
        &self,
        glyph: GlyphCode,
        corner: CornerPosition,
        correction_height: i32,
    ) -> i32 {
        let Some(info) = self
            .math
            .glyph_info
            .and_then(|info| info.kern_infos)
            .and_then(|infos| infos.get(GlyphId(glyph as u16)))
        else {
            return 0;
        };
        let kern = match corner {
            CornerPosition::TopRight => info.top_right,
            CornerPosition::TopLeft => info.top_left,
            CornerPosition::BottomRight => info.bottom_right,
            CornerPosition::BottomLeft => info.bottom_left,
        };
        let Some(kern) = kern else {
            return 0;
        };
        // Heights are interpreted as "below" meaning "less than or equal", like HarfBuzz.
        let index = (0..kern.count())
            .find(|&i| {
                kern.height(i)
                    .is_some_and(|h| correction_height <= h.value as i32)
            })
            .unwrap_or(kern.count());
        kern.kern(index).map_or(0, |k| k.value as i32)
    }
}

fn try_base_glyph(
    shaper: &Shaper,
    glyph: u32,
    horizontal: bool,
    target_size: i32,
    options: ShapeOptions,
) -> Option<Vec<MathGlyph>> {
    let glyph = shaper.glyphs_from_index(glyph, options)[0];

    let advance = if horizontal {
        glyph.advance_width
    } else {
        -glyph.extents.height()
    };

    if advance >= target_size {
        Some(vec![glyph])
    } else {
        None
    }
}

/// The first size variant that is at least `target_size` long.
fn try_variant(
    shaper: &Shaper,
    glyph: u32,
    horizontal: bool,
    target_size: i32,
    options: ShapeOptions,
) -> Option<Vec<MathGlyph>> {
    let (variant, _) = shaper
        .variants(glyph, horizontal)
        .into_iter()
        .find(|&(_, advance)| advance >= target_size)?;
    Some(shaper.glyphs_from_index(variant, options))
}

/// The last size variant of the glyph, which is the largest one.
fn largest_variant(
    shaper: &Shaper,
    glyph: u32,
    horizontal: bool,
    options: ShapeOptions,
) -> Option<Vec<MathGlyph>> {
    let (variant, _) = shaper.variants(glyph, horizontal).into_iter().last()?;
    Some(shaper.glyphs_from_index(variant, options))
}

fn try_assembly(
    shaper: &Shaper,
    glyph: u32,
    horizontal: bool,
    target_size: i32,
    options: ShapeOptions,
) -> Option<Vec<(Vector<i32>, Vec<MathGlyph>)>> {
    // The font's `MinConnectorOverlap`: parts must overlap by at least this much.
    let min_connector_overlap: i32 = shaper
        .math
        .variants
        .map_or(0, |variants| variants.min_connector_overlap as i32);

    let parts = shaper.assembly_parts(glyph, horizontal);

    let mut full_advance_sum_non_ext: i32 = 0;
    let mut full_advance_sum_ext: i32 = 0;
    let mut part_count_non_ext: u32 = 0;
    let mut part_count_ext: u32 = 0;

    for part in &parts {
        if part.extender {
            full_advance_sum_ext += part.full_advance;
            part_count_ext += 1;
        } else {
            full_advance_sum_non_ext += part.full_advance;
            part_count_non_ext += 1;
        }
    }

    let a = full_advance_sum_non_ext - min_connector_overlap * (part_count_non_ext as i32 - 1);
    let b = full_advance_sum_ext - min_connector_overlap * part_count_ext as i32;
    if b == 0 {
        // there probably is no glyph assembly for this glyph
        return None;
    };
    let repeat_count_ext = ((target_size - a) as f32 / b as f32).ceil().max(0.0) as u32;

    // Total number of parts needed to assemble the glyph including repetitions of extenders.
    let part_count = part_count_non_ext + part_count_ext * repeat_count_ext;

    if part_count == 0 || part_count > 2000 {
        return None;
    }

    let connector_overlap = if part_count >= 2 {
        // First determine the ideal overlap that would get closest to the target
        // size. The following quotient is integer operation and gives the best
        // lower approximation of the actual value with fractional pixels.
        let c = full_advance_sum_non_ext + repeat_count_ext as i32 * full_advance_sum_ext;
        let mut connector_overlap = (c - target_size) / (part_count as i32 - 1);

        // We now consider the constraints on connectors. In general, only the
        // start of the first part and then end of the last part are not connected
        // so it is the minimum of StartConnector_i for all i > 0 and of
        // EndConnector_i for all i < glyphAssembly.part_record_count()-1. However,
        // if the first or last part is an extender then it will be connected too
        // with a copy of itself.
        for (index, part) in parts.iter().enumerate() {
            let will_be_repeated = repeat_count_ext >= 2 && part.extender;
            if index < (part_count_ext + part_count_non_ext - 1) as usize || will_be_repeated {
                connector_overlap = min(connector_overlap, part.end_connector_length);
            }
            if index > 0 || will_be_repeated {
                connector_overlap = min(connector_overlap, part.start_connector_length);
            }
        }
        if connector_overlap < min_connector_overlap {
            return None;
        };
        connector_overlap
    } else {
        0
    };

    let result = parts
        .into_iter()
        // Repeat the extenders `repeat_count_ext` times .
        .flat_map(move |part| {
            let repeat_count = if part.extender { repeat_count_ext } else { 1 } as usize;
            std::iter::repeat_n(part, repeat_count)
        })
        // Offset the each glyph from the previous glyph by the advance of the part minus the
        // connector overlap.
        .scan(/* initial offset */ 0, move |current_offset, part| {
            let delta_offset = part.full_advance - connector_overlap;
            let origin = point_with_offset(*current_offset, horizontal);
            let glyphs = shaper.glyphs_from_index(part.glyph, options);

            if horizontal {
                *current_offset += delta_offset;
            } else {
                *current_offset -= delta_offset;
            }
            Some((origin, glyphs))
        });

    Some(result.collect())
}

#[cfg(test)]
mod test {
    use super::*;

    fn font() -> Shaper<'static> {
        Shaper::from_slice(include_bytes!("../../tests/testfiles/latinmodern-math.otf"), 0)
            .expect("test font has a MATH table")
    }

    fn options(script_level: u8) -> ShapeOptions {
        ShapeOptions {
            script_level,
            ..Default::default()
        }
    }

    #[test]
    fn ssty_applies_when_shaping_text() {
        let font = font();
        let normal = font.shape("\u{2032}", options(0));
        let script = font.shape("\u{2032}", options(1));
        assert_ne!(normal[0].glyph_code, script[0].glyph_code);
    }

    #[test]
    fn ssty_applies_to_glyph_indices() {
        let font = font();
        let glyph = font.shape("\u{2032}", options(0))[0].glyph_code;
        let script_glyph = font.shape("\u{2032}", options(1))[0].glyph_code;
        assert_eq!(font.glyph(glyph, options(0)).glyph_code, glyph);
        assert_eq!(font.glyph(glyph, options(1)).glyph_code, script_glyph);
    }

    #[test]
    fn assembly_respects_min_connector_overlap() {
        let font = font();
        let overlap = font.math.variants.unwrap().min_connector_overlap as i32;
        assert!(overlap > 0, "test needs a font with a connector overlap");
        let brace = font.shape("{", options(0))[0].glyph_code;
        let parts = font.stretch(brace, false, 100_000, options(0));
        let assembly = font.assembly_parts(brace, false);
        assert!(parts.len() > 2 && !assembly.is_empty());
        // consecutive parts overlap by at least the minimum
        for pair in parts.windows(2) {
            let (a_off, a) = &pair[0];
            let (b_off, _) = &pair[1];
            let step = a_off.y - b_off.y;
            let part = assembly.iter().find(|p| p.glyph == a.glyph_code).unwrap();
            assert!(part.full_advance - step >= overlap, "overlap too small");
        }
    }
}
