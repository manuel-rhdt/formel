//! A [`MathFont`] implementation on the fontations stack: [harfrust](https://docs.rs/harfrust)
//! for shaping and [read-fonts](https://docs.rs/read-fonts) for the OpenType `MATH`, `GSUB` and
//! `GDEF` tables, glyph metrics and outlines.
//!
//! No type of those crates appears in this module's public API, so upgrading them (they release
//! breaking versions often) does not break callers.
//!
//! # Error handling
//!
//! Every read-fonts accessor returns a `Result`. A damaged table is treated as an absent one: a
//! constant reads as 0, a glyph has no variants, kerning, italic correction or substitution. Shaping
//! errors give an empty run and are reported by [`Shaper::take_shape_error`]; nothing panics on
//! malformed data.

use std::cell::RefCell;
#[cfg(feature = "svg")]
use std::fmt::Write;
use std::sync::Arc;

use harfrust::{Buffer, Direction, Feature, Script, ShapeOptions as HbOptions, ShaperFont};
use read_fonts::TableProvider;
use read_fonts::model::Font;
#[cfg(feature = "svg")]
use read_fonts::model::glyph::outline::{DrawMemory, DrawOptions, PathSink};
use read_fonts::tables::gsub::{AlternateSubst, SingleSubst, SubstitutionSubtables};
use read_fonts::tables::math::{
    Math, MathConstant as RawConstant, MathGlyphConstruction, MathKernCorner, StretchAxis,
};
use read_fonts::types::{GlyphId, Tag};

use crate::host::{
    CornerPosition, Extents, GlyphCode, MathConstant, MathFont, MathGlyph, ShapeOptions, Vector,
};

/// The font bytes, shared with the parsed font.
type FontData = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// A font with a `MATH` table.
pub struct Shaper {
    font: Font,
    data: FontData,
    face_index: u32,
    buffer: RefCell<Option<Buffer>>,
    shape_error: RefCell<Option<String>>,
}

impl std::fmt::Debug for Shaper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shaper")
            .field("face_index", &self.face_index)
            .finish_non_exhaustive()
    }
}

impl Shaper {
    /// Parses font data (copying it) and creates a shaper for the font at `face_index` (nonzero
    /// only for collections such as `.ttc`). Returns `None` if the data cannot be parsed or the
    /// font has no `MATH` table.
    pub fn from_slice(data: &[u8], face_index: u32) -> Option<Shaper> {
        Self::from_shared(Arc::new(data.to_vec()), face_index)
    }

    /// Like [`from_slice`](Self::from_slice) for data that lives for the whole program (for
    /// example `include_bytes!`), without copying it.
    pub fn from_static(data: &'static [u8], face_index: u32) -> Option<Shaper> {
        Self::from_shared(Arc::new(data), face_index)
    }

    /// Like [`from_slice`](Self::from_slice), taking ownership of the data.
    pub fn from_vec(data: Vec<u8>, face_index: u32) -> Option<Shaper> {
        Self::from_shared(Arc::new(data), face_index)
    }

    fn from_shared(data: FontData, face_index: u32) -> Option<Shaper> {
        let font = Font::new(data.clone(), face_index)?;
        let shaper = Shaper {
            font,
            data,
            face_index,
            buffer: RefCell::new(Some(Buffer::new())),
            shape_error: RefCell::new(None),
        };
        shaper.math()?;
        Some(shaper)
    }

    /// The bytes of the font file (a collection as a whole; see [`face_index`](Self::face_index)).
    pub fn font_data(&self) -> &[u8] {
        (*self.data).as_ref()
    }

    /// The index of the font within [`font_data`](Self::font_data).
    pub fn face_index(&self) -> u32 {
        self.face_index
    }

    /// The error of the last failed shaping call, if any, clearing it. A failed call returns an
    /// empty run instead of panicking.
    pub fn take_shape_error(&self) -> Option<String> {
        self.shape_error.borrow_mut().take()
    }

    /// The `MATH` table. `None` only for a table that cannot be read at all.
    fn math(&self) -> Option<Math<'_>> {
        self.font.tables().math().ok()
    }

    /// The value of a MATH table constant.
    pub fn math_constant(&self, c: MathConstant) -> i32 {
        use MathConstant::*;
        let Some(math) = self.math() else {
            return 0;
        };
        let Ok(constants) = math.math_constants() else {
            return 0;
        };
        // Cambria Math has incorrect values for these two constants. Like HarfBuzz (and,
        // apparently, Microsoft's implementation) we swap them when we detect that font.
        // See https://github.com/harfbuzz/harfbuzz/issues/4653
        let c = match c {
            DisplayOperatorMinHeight if math.has_swapped_min_heights() => {
                DelimitedSubFormulaMinHeight
            }
            DelimitedSubFormulaMinHeight if math.has_swapped_min_heights() => {
                DisplayOperatorMinHeight
            }
            c => c,
        };
        // formel's constants are numbered like the OpenType ones (see the `constants_match` test).
        RawConstant::new(c as u8).map_or(0, |c| constants.constant(c))
    }

    fn features(options: ShapeOptions) -> Vec<Feature> {
        options
            .gsub_features()
            .into_iter()
            .map(|(tag, value)| Feature::new(Tag::new(&tag), value, ..))
            .collect()
    }

    fn shape_str(&self, string: &str, options: ShapeOptions) -> Vec<MathGlyph> {
        let mut buffer = self.buffer.borrow_mut().take().unwrap_or_default();
        buffer.clear();
        buffer.push_str(string);
        // The ISO 15924 tag `Math` selects the OpenType `math` script (harfrust's own
        // `Script::MATH`, `Zmth`, would map to `zmth` instead).
        buffer.set_script(Script::from_iso15924_tag(Tag::new(b"Math")));
        buffer.set_direction(Direction::LeftToRight);

        let features = Self::features(options);
        let font = ShaperFont::new(&self.font);
        let result = harfrust::shape(&font, &mut buffer, HbOptions::new().features(&features));
        let glyphs = match result {
            Ok(()) => self.layout_boxes(&buffer),
            Err(error) => {
                *self.shape_error.borrow_mut() = Some(error.to_string());
                Vec::new()
            }
        };
        buffer.clear();
        *self.buffer.borrow_mut() = Some(buffer);
        glyphs
    }

    fn layout_boxes(&self, buffer: &Buffer) -> Vec<MathGlyph> {
        buffer
            .glyph_positions()
            .iter()
            .zip(buffer.glyph_infos())
            .map(|(pos, info)| {
                self.math_glyph(
                    info.glyph_id,
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

    fn is_mark(&self, glyph: u32) -> bool {
        const MARK_CLASS: u16 = 3;
        self.font
            .tables()
            .gdef()
            .ok()
            .and_then(|gdef| gdef.glyph_class_def()?.ok())
            .is_some_and(|classes| classes.get(GlyphId::new(glyph)) as u16 == MARK_CLASS)
    }

    fn hor_advance(&self, glyph: u32) -> i32 {
        self.font
            .glyph_metrics()
            .h_advance(GlyphId::new(glyph))
            .round() as i32
    }

    /// The glyph with the given glyph index (bypassing the cmap), after applying the GSUB
    /// features requested by `options`.
    fn glyphs_from_index(&self, glyph_index: u32, options: ShapeOptions) -> Vec<MathGlyph> {
        let glyph = self.substitute(glyph_index, options);
        let advance = if self.is_mark(glyph) {
            0
        } else {
            self.hor_advance(glyph)
        };
        vec![self.math_glyph(glyph, 0, advance, Vector { x: 0, y: 0 })]
    }

    /// Applies the single and alternate substitutions of the features in `options` to one glyph.
    ///
    /// A table that cannot be read means no substitution: damage in the script or feature list
    /// ends the walk, damage in one lookup skips that lookup.
    fn substitute(&self, glyph: u32, options: ShapeOptions) -> u32 {
        let wanted: Vec<(Tag, u16)> = options
            .gsub_features()
            .into_iter()
            .map(|(tag, value)| (Tag::new(&tag), value.min(u16::MAX as u32) as u16))
            .collect();
        if wanted.is_empty() {
            return glyph;
        }
        self.try_substitute(glyph, &wanted).unwrap_or(glyph)
    }

    fn try_substitute(&self, glyph: u32, wanted: &[(Tag, u16)]) -> Option<u32> {
        let mut glyph = GlyphId::new(glyph);
        let gsub = self.font.tables().gsub().ok()?;
        let scripts = gsub.script_list().ok()?;
        let script_index = scripts
            .index_for_tag(Tag::new(b"math"))
            .or_else(|| scripts.index_for_tag(Tag::new(b"DFLT")))?;
        let script = scripts.get(script_index).ok()?.element;
        let language = script.default_lang_sys()?.ok()?;
        let features = gsub.feature_list().ok()?;
        let lookups = gsub.lookup_list().ok()?;

        // (lookup index, feature value)
        let mut selected: Vec<(u16, u16)> = Vec::new();
        for feature_index in language.feature_indices() {
            let Some(record) = features.feature_records().get(feature_index.get() as usize) else {
                continue;
            };
            let Some(&(_, value)) = wanted.iter().find(|(tag, _)| *tag == record.feature_tag())
            else {
                continue;
            };
            let Ok(feature) = record.feature(features.offset_data()) else {
                continue;
            };
            selected.extend(
                feature
                    .lookup_list_indices()
                    .iter()
                    .map(|i| (i.get(), value)),
            );
        }
        selected.sort_by_key(|&(index, _)| index);
        selected.dedup();

        for (lookup_index, value) in selected {
            let Ok(lookup) = lookups.lookups().get(lookup_index as usize) else {
                continue;
            };
            let Ok(subtables) = lookup.subtables() else {
                continue;
            };
            let replacement = match subtables {
                SubstitutionSubtables::Single(subtables) => subtables
                    .iter()
                    .filter_map(Result::ok)
                    .find_map(|single| single_substitution(&single, glyph)),
                SubstitutionSubtables::Alternate(subtables) => subtables
                    .iter()
                    .filter_map(Result::ok)
                    .find_map(|alt| alternate_substitution(&alt, glyph, value)),
                _ => None,
            };
            if let Some(replacement) = replacement {
                glyph = replacement;
            }
        }
        Some(glyph.to_u32())
    }

    /// The ink extents of a glyph in font units.
    fn extents(&self, glyph: u32) -> Extents<i32> {
        match self.font.glyph_metrics().extents(GlyphId::new(glyph)) {
            Some(e) => Extents {
                left_side_bearing: e.x_bearing.round() as i32,
                width: e.width.round() as i32,
                ascent: e.y_bearing.round() as i32,
                descent: (e.height - e.y_bearing).round() as i32,
            },
            None => Extents::default(),
        }
    }

    fn math_glyph(
        &self,
        glyph: u32,
        cluster: u32,
        advance_width: i32,
        offset: Vector<i32>,
    ) -> MathGlyph {
        let extents = self.extents(glyph);
        let gid = GlyphId::new(glyph);
        let info = self.math().and_then(|math| math.math_glyph_info().ok());
        MathGlyph {
            glyph_code: glyph,
            cluster,
            offset,
            advance_width,
            extents,
            italic_correction: info
                .as_ref()
                .and_then(|info| info.math_italics_correction_info()?.ok())
                .and_then(|table| table.correction(gid))
                .unwrap_or(0),
            // Like HarfBuzz: glyphs without an entry are attached at half their advance. Marks
            // have no advance (and their ink is usually left of the origin), so they are
            // attached at the center of their ink instead.
            top_accent_attachment: info
                .as_ref()
                .and_then(|info| info.math_top_accent_attachment()?.ok())
                .and_then(|table| table.attachment(gid))
                .unwrap_or_else(|| {
                    if advance_width == 0 {
                        extents.left_side_bearing + extents.width / 2
                    } else {
                        self.hor_advance(glyph) / 2
                    }
                }),
        }
    }

    fn construction(&self, glyph: u32, horizontal: bool) -> Option<MathGlyphConstruction<'_>> {
        let axis = if horizontal {
            StretchAxis::Horizontal
        } else {
            StretchAxis::Vertical
        };
        self.math()?
            .math_variants()
            .ok()?
            .glyph_construction(GlyphId::new(glyph), axis)
    }

    /// The size variants of a glyph in increasing size, as (glyph, advance) pairs.
    fn variants(&self, glyph: u32, horizontal: bool) -> Vec<(u32, i32)> {
        self.construction(glyph, horizontal)
            .map(|c| {
                c.math_glyph_variant_records()
                    .iter()
                    .map(|v| {
                        (
                            v.variant_glyph().to_u32(),
                            v.advance_measurement().to_u16() as i32,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn assembly_parts(&self, glyph: u32, horizontal: bool) -> Vec<Part> {
        self.construction(glyph, horizontal)
            .and_then(|c| c.glyph_assembly()?.ok())
            .map(|a| {
                a.part_records()
                    .iter()
                    .map(|p| Part {
                        glyph: p.glyph_id().to_u32(),
                        start_connector_length: p.start_connector_length().to_u16() as i32,
                        end_connector_length: p.end_connector_length().to_u16() as i32,
                        full_advance: p.full_advance().to_u16() as i32,
                        extender: p
                            .part_flags()
                            .contains(read_fonts::tables::math::PartFlags::EXTENDER_FLAG),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `GlyphAssembly.italicsCorrection` of the assembly of a glyph, in font units; 0 without
    /// assembly.
    fn assembly_italic_correction(&self, glyph: u32, horizontal: bool) -> i32 {
        self.construction(glyph, horizontal)
            .and_then(|c| c.glyph_assembly()?.ok())
            .map_or(0, |assembly| {
                assembly.italics_correction().value().to_i16() as i32
            })
    }

    /// The font's `MinConnectorOverlap`: assembly parts must overlap by at least this much.
    fn min_connector_overlap(&self) -> i32 {
        self.math()
            .and_then(|math| math.math_variants().ok())
            .map_or(0, |variants| {
                variants.min_connector_overlap().to_u16() as i32
            })
    }

    /// The outline of a glyph as SVG path data in font units (y up), or an empty string if the
    /// glyph has no outline.
    #[cfg(feature = "svg")]
    pub(crate) fn glyph_path_data(&self, glyph: u32) -> String {
        let mut builder = PathBuilder::default();
        if let Some(outline) = self.font.outline_glyphs().get(GlyphId::new(glyph)) {
            // A glyph that cannot be drawn is left out, like a glyph without outline.
            if outline
                .draw(&DrawOptions::new(), DrawMemory::Scratch, &mut builder)
                .is_err()
            {
                builder.data.clear();
            }
        }
        builder.data.trim_end().to_owned()
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
        self.construction(glyph, horizontal).is_some_and(|c| {
            c.variant_count() > 0
                || c.glyph_assembly()
                    .and_then(Result::ok)
                    .is_some_and(|a| a.part_count() > 0)
        })
    }
}

/// The glyph a `SingleSubst` subtable replaces `glyph` with, if it covers it. Besides the formats
/// 1 and 2 of OpenType 1.9 this reads the 24-bit-glyph formats 3 and 4 of ISO/IEC 14496-22:2026.
fn single_substitution(single: &SingleSubst<'_>, glyph: GlyphId) -> Option<GlyphId> {
    let id = glyph.to_u32();
    match single {
        SingleSubst::Format1(f) => {
            f.coverage().ok()?.get(glyph)?;
            // the sum wraps around in 16 bits
            Some(GlyphId::new(
                (id as i32 + f.delta_glyph_id() as i32) as u16 as u32,
            ))
        }
        SingleSubst::Format2(f) => {
            let index = f.coverage().ok()?.get(glyph)?;
            Some(GlyphId::from(
                f.substitute_glyph_ids().get(index as usize)?.get(),
            ))
        }
        SingleSubst::Format3(f) => {
            f.coverage().ok()?.get(glyph)?;
            // the sum wraps around in 24 bits
            Some(GlyphId::new(
                (id as i32 + f.delta_glyph_id().to_i32()) as u32 & 0xFF_FFFF,
            ))
        }
        SingleSubst::Format4(f) => {
            let index = f.coverage().ok()?.get(glyph)?;
            Some(GlyphId::new(
                f.substitute_glyph_ids().get(index as usize)?.get().to_u32(),
            ))
        }
    }
}

/// The `alternate` (1-based, as in a feature value) alternate of `glyph` in an `AlternateSubst`
/// subtable.
fn alternate_substitution(
    alt: &AlternateSubst<'_>,
    glyph: GlyphId,
    alternate: u16,
) -> Option<GlyphId> {
    let alternate = alternate.checked_sub(1)? as usize;
    match alt {
        AlternateSubst::Format1(f) => {
            let index = f.coverage().ok()?.get(glyph)?;
            let set = f.alternate_sets().get(index as usize).ok()?;
            Some(GlyphId::from(
                set.alternate_glyph_ids().get(alternate)?.get(),
            ))
        }
        AlternateSubst::Format2(f) => {
            let index = f.coverage().ok()?.get(glyph)?;
            let set = f.alternate_sets().get(index as usize).ok()?;
            Some(GlyphId::new(
                set.alternate_glyph_ids().get(alternate)?.get().to_u32(),
            ))
        }
    }
}

/// Collects a glyph outline as SVG path data.
#[cfg(feature = "svg")]
#[derive(Debug, Default)]
struct PathBuilder {
    data: String,
}

#[cfg(feature = "svg")]
impl PathBuilder {
    fn push(&mut self, command: std::fmt::Arguments) {
        let _ = write!(self.data, "{} ", command);
    }
}

#[cfg(feature = "svg")]
impl PathSink for PathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.push(format_args!("M{} {}", x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.push(format_args!("L{} {}", x, y));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.push(format_args!("Q{} {} {} {}", cx, cy, x, y));
    }
    fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.push(format_args!("C{} {} {} {} {} {}", c1x, c1y, c2x, c2y, x, y));
    }
    fn close(&mut self) {
        self.push(format_args!("Z"));
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

impl MathFont for Shaper {
    fn units_per_em(&self) -> u32 {
        self.font.units_per_em() as u32
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
        self.stretch_with_italic_correction(glyph, horizontal, target_size, options)
            .0
    }

    fn stretch_with_italic_correction(
        &self,
        glyph: GlyphCode,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> (Vec<(Vector<i32>, MathGlyph)>, i32) {
        match self.stretch_parts(glyph, horizontal, target_size, options) {
            Stretched::Single(glyphs) => {
                // a variant (or the base glyph): its own correction
                let italic_correction = match glyphs.as_slice() {
                    [only] => only.italic_correction,
                    _ => 0,
                };
                let parts = glyphs
                    .into_iter()
                    .scan(0, |advance, glyph| {
                        let offset = Vector { x: *advance, y: 0 };
                        *advance += glyph.advance_width;
                        Some((offset, glyph))
                    })
                    .collect();
                (parts, italic_correction)
            }
            Stretched::Assembly(parts) => {
                // an assembly: the one of the assembly, not of its parts
                let italic_correction = self.assembly_italic_correction(glyph, horizontal);
                let parts = parts
                    .into_iter()
                    .flat_map(|(origin, glyphs)| glyphs.into_iter().map(move |g| (origin, g)))
                    .collect();
                (parts, italic_correction)
            }
        }
    }

    fn math_kerning(
        &self,
        glyph: GlyphCode,
        corner: CornerPosition,
        correction_height: i32,
    ) -> i32 {
        let corner = match corner {
            CornerPosition::TopRight => MathKernCorner::TopRight,
            CornerPosition::TopLeft => MathKernCorner::TopLeft,
            CornerPosition::BottomRight => MathKernCorner::BottomRight,
            CornerPosition::BottomLeft => MathKernCorner::BottomLeft,
        };
        self.math()
            .and_then(|math| math.math_glyph_info().ok())
            .and_then(|info| info.math_kern_info()?.ok())
            .and_then(|kerns| kerns.kern(GlyphId::new(glyph), corner))
            .and_then(|kern| kern.kerning(correction_height))
            .unwrap_or(0)
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

/// How an assembly is built for a target size: how often the extenders are repeated and by how
/// much consecutive parts overlap.
#[derive(Debug, Clone, Copy, PartialEq)]
struct AssemblyPlan {
    repeat_count_ext: u32,
    connector_overlap: f32,
}

/// The plan for an assembly of `parts` of at least `target_size`, with parts overlapping by at
/// least `min_connector_overlap` (MathML Core 5.3.1), or `None` if that is impossible.
fn plan_assembly(
    parts: &[Part],
    min_connector_overlap: i32,
    target_size: i32,
) -> Option<AssemblyPlan> {
    let mut full_advance_sum_non_ext: i32 = 0;
    let mut full_advance_sum_ext: i32 = 0;
    let mut part_count_non_ext: u32 = 0;
    let mut part_count_ext: u32 = 0;

    for part in parts {
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
    if b <= 0 {
        // there probably is no glyph assembly for this glyph, or extenders that do not grow it
        return None;
    };
    let repeat_count_ext = ((target_size - a) as f32 / b as f32).ceil().max(0.0) as u32;

    // Total number of parts needed to assemble the glyph including repetitions of extenders.
    let part_count = part_count_non_ext + part_count_ext * repeat_count_ext;

    if part_count == 0 || part_count > 2000 {
        return None;
    }

    let connector_overlap = if part_count >= 2 {
        // First determine the ideal overlap that would get closest to the target size. It is not
        // rounded down to whole units (as the algorithm of MathML Core does, which can make the
        // assembly up to `part_count - 1` units longer than asked), so that the assembly is as long
        // as the target, as in TeX. The positions of the parts are rounded below.
        let c = full_advance_sum_non_ext + repeat_count_ext as i32 * full_advance_sum_ext;
        let mut connector_overlap = (c - target_size) as f32 / (part_count as f32 - 1.0);

        // We now consider the constraints on connectors. In general, only the
        // start of the first part and then end of the last part are not connected
        // so it is the minimum of StartConnector_i for all i > 0 and of
        // EndConnector_i for all i < glyphAssembly.part_record_count()-1. However,
        // if the first or last part is an extender then it will be connected too
        // with a copy of itself.
        for (index, part) in parts.iter().enumerate() {
            let will_be_repeated = repeat_count_ext >= 2 && part.extender;
            if index < (part_count_ext + part_count_non_ext - 1) as usize || will_be_repeated {
                connector_overlap = connector_overlap.min(part.end_connector_length as f32);
            }
            if index > 0 || will_be_repeated {
                connector_overlap = connector_overlap.min(part.start_connector_length as f32);
            }
        }
        if connector_overlap < min_connector_overlap as f32 {
            return None;
        };
        connector_overlap
    } else {
        0.0
    };
    Some(AssemblyPlan {
        repeat_count_ext,
        connector_overlap,
    })
}

/// The longest overlap that the connectors of `parts` allow at every joint, if every joint allows
/// a positive one. Extenders count with both connectors, since they may be joined to themselves.
fn largest_overlap_the_connectors_allow(parts: &[Part]) -> Option<i32> {
    let last = parts.len().checked_sub(1)?;
    let limit = parts
        .iter()
        .enumerate()
        .flat_map(|(index, part)| {
            let end = (index < last || part.extender).then_some(part.end_connector_length);
            let start = (index > 0 || part.extender).then_some(part.start_connector_length);
            [end, start]
        })
        .flatten()
        .min()?;
    (limit > 0).then_some(limit)
}

/// [`plan_assembly`], tolerating a font whose `MinConnectorOverlap` is larger than its parts'
/// connectors can ever overlap by. MathML Core calls such an assembly invalid, and an engine that
/// obeys falls back to the largest size variant, which for a tall delimiter is far too short.
/// Asana Math 0.907 is such a font: `MinConnectorOverlap` is 100, its connectors are 15 to 35 units
/// long. The font's intention is clear (the parts are meant to be joined at their connectors), so
/// the minimum is lowered to what the connectors allow.
///
/// An assembly the font allows is planned exactly as by [`plan_assembly`].
fn plan_assembly_tolerantly(
    parts: &[Part],
    min_connector_overlap: i32,
    target_size: i32,
) -> Option<AssemblyPlan> {
    plan_assembly(parts, min_connector_overlap, target_size).or_else(|| {
        let allowed = largest_overlap_the_connectors_allow(parts)?;
        (allowed < min_connector_overlap)
            .then(|| plan_assembly(parts, allowed, target_size))
            .flatten()
    })
}

fn try_assembly(
    shaper: &Shaper,
    glyph: u32,
    horizontal: bool,
    target_size: i32,
    options: ShapeOptions,
) -> Option<Vec<(Vector<i32>, Vec<MathGlyph>)>> {
    let parts = shaper.assembly_parts(glyph, horizontal);
    let AssemblyPlan {
        repeat_count_ext,
        connector_overlap,
    } = plan_assembly_tolerantly(&parts, shaper.min_connector_overlap(), target_size)?;

    let result = parts
        .into_iter()
        // Repeat the extenders `repeat_count_ext` times .
        .flat_map(move |part| {
            let repeat_count = if part.extender { repeat_count_ext } else { 1 } as usize;
            std::iter::repeat_n(part, repeat_count)
        })
        // Offset each glyph from the previous glyph by the advance of the part minus the
        // connector overlap, rounding the position (not each step, which would add up).
        .scan(0.0f32, move |current_offset, part| {
            let offset = (*current_offset).round() as i32;
            let mut origin = point_with_offset(offset, horizontal);
            let glyphs = shaper.glyphs_from_index(part.glyph, options);
            if !horizontal {
                // MathML Core 5.3.1: a vertical part is drawn so that the bottom of the glyph is at
                // the running position, not its origin. Most fonts draw their parts upwards from
                // the origin (depth 0, no change), but some (Asana Math 0.962) hang them below it.
                origin.y -= glyphs.first().map_or(0, |glyph| glyph.extents.descent);
            }
            let delta_offset = part.full_advance as f32 - connector_overlap;
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

    fn font() -> Shaper {
        Shaper::from_static(
            include_bytes!("../../tests/testfiles/latinmodern-math.otf"),
            0,
        )
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

    fn newcm() -> Shaper {
        // `tests/testfiles/` rather than `web/fonts/`: the crate package excludes `web/`, so a test
        // that reads from there does not compile from the published tarball.
        Shaper::from_static(
            include_bytes!("../../tests/testfiles/newcomputermodern-math.otf"),
            0,
        )
        .expect("NewCM has a MATH table")
    }

    fn with_features(settings: &str) -> ShapeOptions {
        ShapeOptions {
            font_features: crate::host::parse_font_feature_settings(settings).unwrap(),
            ..Default::default()
        }
    }

    #[test]
    fn stylistic_set_changes_doublestruck_letters() {
        let font = newcm();
        // DOUBLE-STRUCK CAPITAL A, N and R
        for c in ["\u{1D538}", "\u{2115}", "\u{211D}"] {
            let default = font.shape(c, ShapeOptions::default())[0].glyph_code;
            let ss03 = font.shape(c, with_features("\"ss03\""))[0].glyph_code;
            assert_ne!(default, ss03, "ss03 must change {c:?}");
            // turning the feature off again gives the default glyph
            let off = font.shape(c, with_features("\"ss03\" off"))[0].glyph_code;
            assert_eq!(default, off);
            // the glyph-index path (used for variants and assemblies) agrees with shaping
            assert_eq!(
                font.glyph(default, with_features("\"ss03\"")).glyph_code,
                ss03
            );
        }
    }

    #[test]
    fn user_features_keep_builtin_ones() {
        let font = font();
        let options = ShapeOptions {
            script_level: 1,
            font_features: crate::host::parse_font_feature_settings("\"ss01\"").unwrap(),
            ..Default::default()
        };
        let plain = font.shape("\u{2032}", options)[0].glyph_code;
        assert_eq!(
            plain,
            font.shape("\u{2032}", self::options(1))[0].glyph_code
        );
        // a user setting of ssty overrides the one derived from math-depth
        let off = ShapeOptions {
            font_features: crate::host::parse_font_feature_settings("\"ssty\" 0").unwrap(),
            ..options
        };
        assert_eq!(
            font.shape("\u{2032}", off)[0].glyph_code,
            font.shape("\u{2032}", self::options(0))[0].glyph_code
        );
    }

    #[test]
    fn assembly_respects_min_connector_overlap() {
        let font = font();
        let overlap = font.min_connector_overlap();
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

    macro_rules! all_constants {
        ($($name:ident),+ $(,)?) => { [$(MathConstant::$name),+] };
    }

    /// formel's `MathConstant` is mapped onto OpenType's by its discriminant.
    #[test]
    fn constants_match_the_opentype_numbering() {
        let ours = all_constants!(
            ScriptPercentScaleDown,
            ScriptScriptPercentScaleDown,
            DelimitedSubFormulaMinHeight,
            DisplayOperatorMinHeight,
            MathLeading,
            AxisHeight,
            AccentBaseHeight,
            FlattenedAccentBaseHeight,
            SubscriptShiftDown,
            SubscriptTopMax,
            SubscriptBaselineDropMin,
            SuperscriptShiftUp,
            SuperscriptShiftUpCramped,
            SuperscriptBottomMin,
            SuperscriptBaselineDropMax,
            SubSuperscriptGapMin,
            SuperscriptBottomMaxWithSubscript,
            SpaceAfterScript,
            UpperLimitGapMin,
            UpperLimitBaselineRiseMin,
            LowerLimitGapMin,
            LowerLimitBaselineDropMin,
            StackTopShiftUp,
            StackTopDisplayStyleShiftUp,
            StackBottomShiftDown,
            StackBottomDisplayStyleShiftDown,
            StackGapMin,
            StackDisplayStyleGapMin,
            StretchStackTopShiftUp,
            StretchStackBottomShiftDown,
            StretchStackGapAboveMin,
            StretchStackGapBelowMin,
            FractionNumeratorShiftUp,
            FractionNumeratorDisplayStyleShiftUp,
            FractionDenominatorShiftDown,
            FractionDenominatorDisplayStyleShiftDown,
            FractionNumeratorGapMin,
            FractionNumDisplayStyleGapMin,
            FractionRuleThickness,
            FractionDenominatorGapMin,
            FractionDenomDisplayStyleGapMin,
            SkewedFractionHorizontalGap,
            SkewedFractionVerticalGap,
            OverbarVerticalGap,
            OverbarRuleThickness,
            OverbarExtraAscender,
            UnderbarVerticalGap,
            UnderbarRuleThickness,
            UnderbarExtraDescender,
            RadicalVerticalGap,
            RadicalDisplayStyleVerticalGap,
            RadicalRuleThickness,
            RadicalExtraAscender,
            RadicalKernBeforeDegree,
            RadicalKernAfterDegree,
            RadicalDegreeBottomRaisePercent,
        );
        assert_eq!(ours.len(), RawConstant::ALL.len());
        for (ours, raw) in ours.into_iter().zip(RawConstant::ALL) {
            assert_eq!(format!("{ours:?}"), format!("{raw:?}"));
            assert_eq!(ours as u8, *raw as u8);
        }
    }

    #[test]
    fn well_formed_fonts_are_not_taken_for_cambria() {
        for font in [font(), newcm()] {
            assert!(!font.math().unwrap().has_swapped_min_heights());
        }
        let font = font();
        // A real constant, from the table
        assert_eq!(font.math_constant(MathConstant::ScriptPercentScaleDown), 70);
    }
}

#[cfg(test)]
mod subset_tests;
