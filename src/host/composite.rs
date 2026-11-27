//! Composite fonts: one [`MathFont`] made of several, each used for some codepoints.
//!
//! This is what CSS calls a composite font, made of `@font-face` rules of one family with
//! different `unicode-range` descriptors (CSS Fonts 4, 4.5.1). formel does not read CSS: the host
//! (or the caller of the wasm API) gives the faces in the order in which they are to be tried, and
//! the codepoint ranges as numbers.
//!
//! For each character the first face whose [range](FontFace::unicode_ranges) contains it and
//! whose character map has a glyph for it is used. A character that no face can show is given to
//! the *primary* face, which is the one that tells the units per em and all MATH constants (the
//! "first available font" of MathML Core).
//!
//! # Glyph codes
//!
//! A glyph code of the primary face is that of the face itself. Those of the other faces carry the
//! number of the face in the top 8 bits, so a composite of only a primary face is the same font as
//! that face, and glyph 0 is `.notdef`. A face can therefore have glyph ids below 2²⁴ only (a glyph
//! above that is `.notdef`), and a composite at most 256 faces. A host that draws glyphs itself
//! finds the face and the scale with [`CompositeFont::decode_glyph`].
//!
//! # Scale
//!
//! The metrics of a glyph of another face are scaled to the units per em of the primary face, and by
//! the face's [`size_adjust`](FontFace::size_adjust) (the `size-adjust` descriptor). The `size_adjust`
//! of the primary face scales its glyphs and its MATH constants (but not the percentages among them) and
//! not the em, so `units_per_em` stays that of the primary font. The values that
//! are rounded to whole units are rounded to the nearest, so with a scale other than 1 an assembly of
//! several parts can be a few units off.

use std::ops::RangeInclusive;

use super::font::{
    CornerPosition, Extents, FontFeatures, GlyphCode, MathConstant, MathFont, MathGlyph,
    ShapeOptions, Vector,
};

/// A set of Unicode codepoints, as the `unicode-range` descriptor of CSS. Ranges may overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnicodeRanges {
    ranges: Vec<RangeInclusive<u32>>,
}

impl UnicodeRanges {
    /// All of Unicode (`U+0-10FFFF`), the default of `unicode-range`.
    pub fn all() -> Self {
        UnicodeRanges {
            ranges: vec![0..=0x10FFFF],
        }
    }

    /// The union of `ranges`, each given by its first and last codepoint (both included).
    /// `0x1D49C..=0x1D503` is `U+1D49C-1D503`, and a single codepoint `0xA5..=0xA5`.
    pub fn new(ranges: impl IntoIterator<Item = RangeInclusive<u32>>) -> Self {
        UnicodeRanges {
            ranges: ranges.into_iter().collect(),
        }
    }

    pub fn contains(&self, c: char) -> bool {
        self.ranges.iter().any(|range| range.contains(&(c as u32)))
    }
}

impl Default for UnicodeRanges {
    fn default() -> Self {
        Self::all()
    }
}

/// A font and what it is to be used for in a [`CompositeFont`].
#[derive(Debug, Clone)]
pub struct FontFace<F> {
    pub font: F,
    /// The codepoints the face may be used for (`unicode-range`). Default: all.
    pub unicode_ranges: UnicodeRanges,
    /// Scales the glyphs and all other metrics of the face (`size-adjust` as a factor: 0.9 is 90%),
    /// not the em. Default: 1.
    pub size_adjust: f32,
    /// Features to apply for this face (`font-feature-settings` of `@font-face`). Settings of the
    /// shaping options override them.
    pub font_features: FontFeatures,
}

impl<F> FontFace<F> {
    pub fn new(font: F) -> Self {
        FontFace {
            font,
            unicode_ranges: UnicodeRanges::all(),
            size_adjust: 1.0,
            font_features: FontFeatures::default(),
        }
    }

    pub fn unicode_ranges(mut self, unicode_ranges: UnicodeRanges) -> Self {
        self.unicode_ranges = unicode_ranges;
        self
    }

    pub fn size_adjust(mut self, size_adjust: f32) -> Self {
        self.size_adjust = size_adjust;
        self
    }

    pub fn font_features(mut self, font_features: FontFeatures) -> Self {
        self.font_features = font_features;
        self
    }
}

/// Why a [`CompositeFont`] could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeFontError {
    NoFaces,
    PrimaryOutOfRange,
    /// More than 256 faces: the face is part of the glyph code, in 8 bits.
    TooManyFaces,
    /// A `size_adjust` that is not a positive, finite number.
    InvalidSizeAdjust,
}

impl std::fmt::Display for CompositeFontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CompositeFontError::NoFaces => "a composite font needs at least one face",
            CompositeFontError::PrimaryOutOfRange => "the primary face is not one of the faces",
            CompositeFontError::TooManyFaces => "a composite font has at most 256 faces",
            CompositeFontError::InvalidSizeAdjust => "size-adjust must be a positive number",
        })
    }
}

impl std::error::Error for CompositeFontError {}

/// A glyph of a [`CompositeFont`] found in its face.
#[derive(Debug)]
pub struct DecodedGlyph<'a, F> {
    /// The number of the face, in the order the faces were given.
    pub face: usize,
    pub font: &'a F,
    /// The glyph code in the face.
    pub glyph: GlyphCode,
    /// The factor to scale the face's units to the units per em of the composite font: the ratio of
    /// the units per em, times `size-adjust`. 1 for the primary face.
    pub scale: f32,
}

#[derive(Debug, Clone)]
struct Face<F> {
    face: FontFace<F>,
    /// The face's number in the top 8 bits of its glyph codes: 0 for the primary face.
    tag: u32,
    scale: f64,
}

/// A [`MathFont`] made of several fonts. See the [module](self) documentation.
#[derive(Debug, Clone)]
pub struct CompositeFont<F> {
    /// In the order they are tried.
    faces: Vec<Face<F>>,
    primary: usize,
    /// The face of each tag.
    by_tag: Vec<usize>,
    units_per_em: u32,
}

const TAG_SHIFT: u32 = 24;
const GLYPH_MASK: u32 = (1 << TAG_SHIFT) - 1;

fn scaled(value: i32, scale: f64) -> i32 {
    if scale == 1.0 {
        value
    } else {
        (value as f64 * scale).round() as i32
    }
}

fn unscaled(value: i32, scale: f64) -> i32 {
    if scale == 1.0 {
        value
    } else {
        (value as f64 / scale).round() as i32
    }
}

/// Characters that belong to the one before them: combining marks, variation selectors and the
/// zero width joiners. They are shaped with the face of the character before if it has them.
fn continues_a_cluster(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF
        | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x200C | 0x200D | 0xE0100..=0xE01EF)
}

impl<F: MathFont> CompositeFont<F> {
    /// A composite font of `faces`, which are tried in this order, with `faces[primary]` as the
    /// face for everything the others cannot show, and for the units per em and the MATH constants.
    pub fn new(faces: Vec<FontFace<F>>, primary: usize) -> Result<Self, CompositeFontError> {
        if faces.is_empty() {
            return Err(CompositeFontError::NoFaces);
        }
        if primary >= faces.len() {
            return Err(CompositeFontError::PrimaryOutOfRange);
        }
        if faces.len() > 256 {
            return Err(CompositeFontError::TooManyFaces);
        }
        if faces
            .iter()
            .any(|face| !(face.size_adjust.is_finite() && face.size_adjust > 0.0))
        {
            return Err(CompositeFontError::InvalidSizeAdjust);
        }

        let units_per_em = faces[primary].font.units_per_em().max(1);
        let mut by_tag = vec![primary];
        let faces = faces
            .into_iter()
            .enumerate()
            .map(|(index, face)| {
                let (tag, scale) = if index == primary {
                    // the units are those of this face, but its `size-adjust` scales it like the others
                    (0, face.size_adjust as f64)
                } else {
                    by_tag.push(index);
                    let ratio = units_per_em as f64 / face.font.units_per_em().max(1) as f64;
                    (by_tag.len() as u32 - 1, ratio * face.size_adjust as f64)
                };
                Face { face, tag, scale }
            })
            .collect();
        Ok(CompositeFont {
            faces,
            primary,
            by_tag,
            units_per_em,
        })
    }

    /// The primary face's font.
    pub fn primary(&self) -> &F {
        &self.faces[self.primary].face.font
    }

    /// Finds the face of a glyph code that this font gave out.
    pub fn decode_glyph(&self, code: GlyphCode) -> Option<DecodedGlyph<'_, F>> {
        let index = *self.by_tag.get((code >> TAG_SHIFT) as usize)?;
        let face = &self.faces[index];
        Some(DecodedGlyph {
            face: index,
            font: &face.face.font,
            glyph: code & GLYPH_MASK,
            scale: face.scale as f32,
        })
    }

    fn encode(&self, face: usize, glyph: GlyphCode) -> GlyphCode {
        if glyph > GLYPH_MASK {
            return 0;
        }
        (self.faces[face].tag << TAG_SHIFT) | glyph
    }

    /// The face for `c`, which follows `previous` if it is a combining mark of it.
    fn face_for(&self, c: char, previous: Option<usize>) -> usize {
        if let Some(previous) = previous {
            if continues_a_cluster(c) && self.faces[previous].face.font.has_glyph(c) {
                return previous;
            }
        }
        self.faces
            .iter()
            .position(|face| face.face.unicode_ranges.contains(c) && face.face.font.has_glyph(c))
            .unwrap_or(self.primary)
    }

    /// The shaping options for a face: its features, overridden by those of `options`.
    fn options_for(&self, face: usize, options: ShapeOptions) -> ShapeOptions {
        let defaults = self.faces[face].face.font_features;
        if defaults.is_empty() {
            return options;
        }
        let mut font_features = defaults;
        for (tag, value) in options.font_features.iter() {
            font_features.set(tag, value);
        }
        ShapeOptions {
            font_features,
            ..options
        }
    }

    /// `glyph`, a glyph of the font of `face`, in the units of this font and with its code.
    fn from_face(&self, face: usize, glyph: MathGlyph, cluster_offset: u32) -> MathGlyph {
        let s = self.faces[face].scale;
        MathGlyph {
            glyph_code: self.encode(face, glyph.glyph_code),
            cluster: glyph.cluster + cluster_offset,
            offset: Vector {
                x: scaled(glyph.offset.x, s),
                y: scaled(glyph.offset.y, s),
            },
            advance_width: scaled(glyph.advance_width, s),
            extents: Extents {
                left_side_bearing: scaled(glyph.extents.left_side_bearing, s),
                width: scaled(glyph.extents.width, s),
                ascent: scaled(glyph.extents.ascent, s),
                descent: scaled(glyph.extents.descent, s),
            },
            italic_correction: scaled(glyph.italic_correction, s),
            top_accent_attachment: scaled(glyph.top_accent_attachment, s),
        }
    }

    fn shape_run(
        &self,
        face: usize,
        text: &str,
        range: std::ops::Range<usize>,
        options: ShapeOptions,
        result: &mut Vec<MathGlyph>,
    ) {
        let glyphs = self.faces[face]
            .face
            .font
            .shape(&text[range.clone()], self.options_for(face, options));
        result.extend(
            glyphs
                .into_iter()
                .map(|glyph| self.from_face(face, glyph, range.start as u32)),
        );
    }

    fn stretch_in_face(
        &self,
        glyph: GlyphCode,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> (Vec<(Vector<i32>, MathGlyph)>, i32) {
        let Some(decoded) = self.decode_glyph(glyph) else {
            return (vec![(Vector::default(), MathGlyph::default())], 0);
        };
        let s = self.faces[decoded.face].scale;
        let (parts, italic_correction) = decoded.font.stretch_with_italic_correction(
            decoded.glyph,
            horizontal,
            unscaled(target_size, s),
            self.options_for(decoded.face, options),
        );
        let parts = parts
            .into_iter()
            .map(|(origin, glyph)| {
                let origin = Vector {
                    x: scaled(origin.x, s),
                    y: scaled(origin.y, s),
                };
                (origin, self.from_face(decoded.face, glyph, 0))
            })
            .collect();
        (parts, scaled(italic_correction, s))
    }
}

impl<F: MathFont> MathFont for CompositeFont<F> {
    fn units_per_em(&self) -> u32 {
        self.units_per_em
    }

    fn constant(&self, c: MathConstant) -> i32 {
        let value = self.primary().constant(c);
        match c {
            // percentages do not depend on the size of the font
            MathConstant::ScriptPercentScaleDown
            | MathConstant::ScriptScriptPercentScaleDown
            | MathConstant::RadicalDegreeBottomRaisePercent => value,
            _ => scaled(value, self.faces[self.primary].scale),
        }
    }

    fn shape(&self, text: &str, options: ShapeOptions) -> Vec<MathGlyph> {
        let mut result = Vec::new();
        // the face and the first byte of the run being collected
        let mut run: Option<(usize, usize)> = None;
        let mut previous = None;
        for (offset, c) in text.char_indices() {
            let face = self.face_for(c, previous);
            previous = Some(face);
            match run {
                Some((current, _)) if current == face => {}
                Some((current, start)) => {
                    self.shape_run(current, text, start..offset, options, &mut result);
                    run = Some((face, offset));
                }
                None => run = Some((face, offset)),
            }
        }
        if let Some((face, start)) = run {
            self.shape_run(face, text, start..text.len(), options, &mut result);
        }
        result
    }

    fn glyph(&self, glyph: GlyphCode, options: ShapeOptions) -> MathGlyph {
        let Some(decoded) = self.decode_glyph(glyph) else {
            return MathGlyph::default();
        };
        let glyph = decoded
            .font
            .glyph(decoded.glyph, self.options_for(decoded.face, options));
        self.from_face(decoded.face, glyph, 0)
    }

    fn has_glyph(&self, c: char) -> bool {
        self.faces
            .iter()
            .any(|face| face.face.unicode_ranges.contains(c) && face.face.font.has_glyph(c))
    }

    fn is_stretchable(&self, glyph: GlyphCode, horizontal: bool) -> bool {
        self.decode_glyph(glyph)
            .is_some_and(|decoded| decoded.font.is_stretchable(decoded.glyph, horizontal))
    }

    fn stretch(
        &self,
        glyph: GlyphCode,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> Vec<(Vector<i32>, MathGlyph)> {
        self.stretch_in_face(glyph, horizontal, target_size, options)
            .0
    }

    fn stretch_with_italic_correction(
        &self,
        glyph: GlyphCode,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> (Vec<(Vector<i32>, MathGlyph)>, i32) {
        self.stretch_in_face(glyph, horizontal, target_size, options)
    }

    fn math_kerning(
        &self,
        glyph: GlyphCode,
        corner: CornerPosition,
        correction_height: i32,
    ) -> i32 {
        let Some(decoded) = self.decode_glyph(glyph) else {
            return 0;
        };
        let s = self.faces[decoded.face].scale;
        scaled(
            decoded
                .font
                .math_kerning(decoded.glyph, corner, unscaled(correction_height, s)),
            s,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaper::Shaper;
    use std::rc::Rc;

    const PRIMARY: &[u8] = include_bytes!("../../tests/testfiles/stixtwo-math-subset.otf");
    /// The same glyphs at 2048 units per em, without E, F, x and +.
    const SECOND: &[u8] = include_bytes!("../../tests/testfiles/stixtwo-math-subset-2048.otf");

    type Font = Rc<Shaper>;

    fn primary() -> Font {
        Rc::new(Shaper::from_static(PRIMARY, 0).unwrap())
    }

    fn second() -> Font {
        Rc::new(Shaper::from_static(SECOND, 0).unwrap())
    }

    fn ranges(ranges: &[RangeInclusive<u32>]) -> UnicodeRanges {
        UnicodeRanges::new(ranges.iter().cloned())
    }

    /// The second font for the codepoints in `ranges`, and the primary font for everything else.
    fn composite(ranges: UnicodeRanges) -> CompositeFont<Font> {
        CompositeFont::new(
            vec![
                FontFace::new(second()).unicode_ranges(ranges),
                FontFace::new(primary()),
            ],
            1,
        )
        .unwrap()
    }

    fn tags(glyphs: &[MathGlyph]) -> Vec<u32> {
        glyphs
            .iter()
            .map(|glyph| glyph.glyph_code >> TAG_SHIFT)
            .collect()
    }

    fn options() -> ShapeOptions {
        ShapeOptions::default()
    }

    /// A scale of 1000 / 2048, exactly representable.
    const SCALE: f32 = 0.48828125;

    #[test]
    fn unicode_ranges_contain_what_they_say() {
        let all = UnicodeRanges::all();
        assert!(all.contains('\0') && all.contains('A') && all.contains('\u{10FFFF}'));
        assert_eq!(UnicodeRanges::default(), all);

        let some = ranges(&[0x41..=0x5A, 0xA5..=0xA5, 0x1D49C..=0x1D503]);
        for c in ['A', 'Q', 'Z', '\u{A5}', '\u{1D49C}', '\u{1D503}'] {
            assert!(some.contains(c), "{c:?}");
        }
        for c in ['@', '[', 'a', '\u{A4}', '\u{A6}', '\u{1D49B}', '\u{1D504}'] {
            assert!(!some.contains(c), "{c:?}");
        }
        // ranges may overlap, and an empty set contains nothing
        assert!(ranges(&[0x41..=0x50, 0x48..=0x60]).contains('`'));
        assert!(!UnicodeRanges::new([]).contains('A'));
    }

    #[test]
    fn a_composite_font_needs_sensible_faces() {
        let face = || FontFace::new(primary());
        let err = |faces: Vec<FontFace<Font>>, primary| CompositeFont::new(faces, primary).err();
        assert_eq!(err(vec![], 0), Some(CompositeFontError::NoFaces));
        assert_eq!(
            err(vec![face()], 1),
            Some(CompositeFontError::PrimaryOutOfRange)
        );
        assert_eq!(
            err((0..257).map(|_| face()).collect(), 0),
            Some(CompositeFontError::TooManyFaces)
        );
        assert!(err((0..256).map(|_| face()).collect(), 255).is_none());
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                err(vec![face().size_adjust(bad), face()], 1),
                Some(CompositeFontError::InvalidSizeAdjust),
                "{bad}"
            );
        }
        // the primary face can be adjusted like the others
        assert!(err(vec![face().size_adjust(0.9)], 0).is_none());
        assert!(err(vec![face().size_adjust(0.9), face()], 1).is_none());
    }

    /// With only a primary face the composite is that font: same glyph codes, same everything.
    #[test]
    fn a_composite_of_one_face_is_that_face() {
        let font = primary();
        let composite = CompositeFont::new(vec![FontFace::new(primary())], 0).unwrap();
        let level = |script_level| ShapeOptions {
            script_level,
            ..Default::default()
        };
        for text in [
            "AVT",
            "\u{1D453}A",
            "A\u{301}",
            "E+x z",
            "\u{222B}\u{221A}",
            "",
        ] {
            for options in [level(0), level(1), level(2)] {
                assert_eq!(
                    composite.shape(text, options),
                    font.shape(text, options),
                    "{text:?}"
                );
            }
        }
        assert_eq!(composite.units_per_em(), font.units_per_em());
        for c in [MathConstant::AxisHeight, MathConstant::RadicalRuleThickness] {
            assert_eq!(composite.constant(c), font.constant(c));
        }
        for c in ['A', 'z', '\u{301}'] {
            assert_eq!(composite.has_glyph(c), font.has_glyph(c));
        }
        let integral = font.shape("\u{222B}", options())[0].glyph_code;
        let a = font.shape("A", options())[0].glyph_code;
        for code in [0, a, integral] {
            assert_eq!(composite.glyph(code, level(1)), font.glyph(code, level(1)));
            for horizontal in [false, true] {
                assert_eq!(
                    composite.is_stretchable(code, horizontal),
                    font.is_stretchable(code, horizontal)
                );
                for target in [0, 900, 1500, 4000, 20_000] {
                    assert_eq!(
                        composite.stretch_with_italic_correction(
                            code,
                            horizontal,
                            target,
                            options()
                        ),
                        font.stretch_with_italic_correction(code, horizontal, target, options())
                    );
                }
            }
            for corner in [CornerPosition::TopRight, CornerPosition::BottomRight] {
                for height in [-100, 0, 300, 5000] {
                    assert_eq!(
                        composite.math_kerning(code, corner, height),
                        font.math_kerning(code, corner, height)
                    );
                }
            }
        }
    }

    /// A is in the range and in the second font; V is outside the range; T is both; E is in the range
    /// but the second font has no E.
    #[test]
    fn a_character_comes_from_the_first_face_that_has_it() {
        let composite = composite(ranges(&[0x41..=0x54]));
        let glyphs = composite.shape("AVTE", options());
        assert_eq!(glyphs.len(), 4);
        assert_eq!(tags(&glyphs), [1, 0, 1, 0]);
        // the glyph codes are those of the fonts, with the number of the face on top
        let (first, second) = (primary(), second());
        assert_eq!(
            glyphs[0].glyph_code & GLYPH_MASK,
            second.shape("A", options())[0].glyph_code
        );
        assert_eq!(
            glyphs[1].glyph_code,
            first.shape("V", options())[0].glyph_code
        );
        assert_eq!(
            glyphs[3].glyph_code,
            first.shape("E", options())[0].glyph_code
        );
    }

    #[test]
    fn the_faces_are_tried_in_the_order_given() {
        // Three faces that all have an A: the second font (scale 1000 / 2048), the primary font as a
        // face of its own, and the primary face. The scale of the face that wins tells which it is.
        let winner_of_a = |order: [Font; 3], primary: usize| {
            let composite = CompositeFont::new(order.map(FontFace::new).to_vec(), primary).unwrap();
            let code = composite.shape("A", options())[0].glyph_code;
            let decoded = composite.decode_glyph(code).unwrap();
            (decoded.face, decoded.scale)
        };
        let (a, b, c) = (second(), primary(), primary());
        assert_eq!(
            winner_of_a([a.clone(), b.clone(), c.clone()], 2),
            (0, SCALE)
        );
        assert_eq!(winner_of_a([b.clone(), a.clone(), c.clone()], 2), (0, 1.0));
        assert_eq!(
            winner_of_a([c.clone(), b.clone(), a.clone()], 0),
            (0, 1.0),
            "the primary first"
        );
        // when the first face cannot show a character, the next one does
        let composite = CompositeFont::new(
            vec![FontFace::new(a), FontFace::new(b), FontFace::new(c)],
            2,
        )
        .unwrap();
        let code = composite.shape("E", options())[0].glyph_code;
        assert_eq!(
            composite.decode_glyph(code).unwrap().face,
            1,
            "the second font has no E"
        );
    }

    #[test]
    fn a_face_is_not_used_outside_its_ranges_even_if_it_has_the_glyph() {
        let composite = composite(ranges(&[0x41..=0x41]));
        // the second font has T, but not in the range
        assert_eq!(tags(&composite.shape("AT", options())), [1, 0]);
    }

    #[test]
    fn a_character_no_face_has_is_notdef_of_the_primary() {
        let composite = composite(ranges(&[0x41..=0x7A]));
        for text in ["z", "B", "\u{20AC}"] {
            let glyphs = composite.shape(text, options());
            assert_eq!(glyphs.len(), 1, "{text:?}");
            assert_eq!(glyphs[0].glyph_code, 0, "{text:?}");
            assert!(!composite.has_glyph(text.chars().next().unwrap()));
        }
        assert!(composite.has_glyph('A') && composite.has_glyph('E'));
    }

    /// A combining mark is shaped with the face of the character before it, if it has the mark.
    #[test]
    fn a_combining_mark_follows_its_base_character() {
        let composite = composite(ranges(&[0x41..=0x41]));
        // U+0301 is outside the range, but the second font has it and A is from the second font
        assert_eq!(tags(&composite.shape("A\u{301}", options())), [1, 1]);
        // after a character of the primary face it is a character of its own, in no range
        assert_eq!(tags(&composite.shape("V\u{301}", options())), [0, 0]);
        // and a mark that the face of the base does not have goes to the first face that does
        let only_a = CompositeFont::new(
            vec![
                FontFace::new(second()).unicode_ranges(ranges(&[0x41..=0x41])),
                FontFace::new(primary()),
            ],
            1,
        )
        .unwrap();
        assert!(only_a.has_glyph('\u{301}'));
    }

    /// `cluster` is the byte offset in the whole text, as for a single font.
    #[test]
    fn clusters_count_bytes_of_the_whole_text() {
        let composite = composite(ranges(&[0x41..=0x41, 0x1D453..=0x1D453]));
        let glyphs = composite.shape("V\u{1D453}AV", options());
        assert_eq!(
            glyphs.iter().map(|g| g.cluster).collect::<Vec<_>>(),
            [0, 1, 5, 6]
        );
        assert_eq!(tags(&glyphs), [0, 1, 1, 0]);
    }

    /// The glyphs of the second font, which has 2048 units per em, come in units of the primary
    /// face: the font is the same, so they are those of the primary face, give or take a rounding.
    #[test]
    fn glyphs_of_another_face_are_scaled_to_the_units_per_em_of_the_primary() {
        let composite = composite(ranges(&[
            0x41..=0x7A,
            0x300..=0x36F,
            0x1D400..=0x1D7FF,
            0x222B..=0x222B,
        ]));
        let reference = primary();
        for text in ["A", "T", "\u{1D453}", "\u{222B}", "\u{301}"] {
            let ours = composite.shape(text, options())[0];
            let theirs = reference.shape(text, options())[0];
            assert_eq!(
                ours.glyph_code >> TAG_SHIFT,
                1,
                "{text:?} is from the second face"
            );
            let near = |a: i32, b: i32, what: &str| {
                assert!((a - b).abs() <= 1, "{text:?} {what}: {a} against {b}");
            };
            near(ours.advance_width, theirs.advance_width, "advance");
            near(ours.offset.x, theirs.offset.x, "offset x");
            near(ours.offset.y, theirs.offset.y, "offset y");
            near(
                ours.extents.left_side_bearing,
                theirs.extents.left_side_bearing,
                "bearing",
            );
            near(ours.extents.width, theirs.extents.width, "width");
            near(ours.extents.ascent, theirs.extents.ascent, "ascent");
            near(ours.extents.descent, theirs.extents.descent, "descent");
            near(
                ours.italic_correction,
                theirs.italic_correction,
                "italic correction",
            );
            near(
                ours.top_accent_attachment,
                theirs.top_accent_attachment,
                "top accent",
            );
        }
        // values the tests know: A is 1470 wide in the second font, and the italic f has 41
        let a = composite.shape("A", options())[0];
        assert_eq!(a.advance_width, (1470.0 * SCALE as f64).round() as i32);
        assert_eq!(
            composite.shape("\u{1D453}", options())[0].italic_correction,
            20
        );
        // the units and constants are those of the primary face
        assert_eq!(composite.units_per_em(), 1000);
        assert_eq!(
            composite.constant(MathConstant::AxisHeight),
            reference.constant(MathConstant::AxisHeight)
        );
        assert_eq!(composite.constant(MathConstant::AxisHeight), 258);
    }

    #[test]
    fn size_adjust_scales_the_glyphs_of_a_face() {
        let advance = |size_adjust: f32| {
            let composite = CompositeFont::new(
                vec![
                    FontFace::new(second()).size_adjust(size_adjust),
                    FontFace::new(primary()),
                ],
                1,
            )
            .unwrap();
            let glyph = composite.shape("A", options())[0];
            (
                composite.units_per_em(),
                glyph.advance_width,
                composite.constant(MathConstant::AxisHeight),
            )
        };
        let (_, normal, _) = advance(1.0);
        assert_eq!(normal, 718);
        for (factor, expected) in [(0.5, 359), (0.9, 646), (2.0, 1436)] {
            let (upem, width, axis) = advance(factor);
            assert_eq!(width, expected, "{factor}");
            assert_eq!((upem, axis), (1000, 258), "the primary face is not scaled");
        }
    }

    /// The kerning of a glyph of the second face is asked at a height in its units, and the answer
    /// is in the units of the composite: the same as for the primary face.
    #[test]
    fn math_kerning_is_converted_in_both_directions() {
        let composite = composite(ranges(&[0x41..=0x41]));
        let reference = primary();
        let ours = composite.shape("A", options())[0].glyph_code;
        let theirs = reference.shape("A", options())[0].glyph_code;
        assert_eq!(ours >> TAG_SHIFT, 1);
        // the bands of the top right corner of A: [252, 352] at 1000 units per em, kerns 0, -18, -66
        for (height, kern) in [(-50, 0), (100, 0), (300, -18), (400, -66), (5000, -66)] {
            assert_eq!(
                reference.math_kerning(theirs, CornerPosition::TopRight, height),
                kern
            );
            assert_eq!(
                composite.math_kerning(ours, CornerPosition::TopRight, height),
                kern,
                "{height}"
            );
        }
        assert_eq!(
            composite.math_kerning(ours, CornerPosition::BottomLeft, 300),
            0
        );
        assert_eq!(
            composite.math_kerning(0x7F00_0000, CornerPosition::TopRight, 300),
            0,
            "unknown face"
        );
    }

    fn ink(parts: &[(Vector<i32>, MathGlyph)]) -> (i32, i32) {
        let top = parts
            .iter()
            .map(|(o, g)| -(o.y + g.offset.y) + g.extents.ascent)
            .max()
            .unwrap();
        let bottom = parts
            .iter()
            .map(|(o, g)| -(o.y + g.offset.y) - g.extents.descent)
            .min()
            .unwrap();
        (top, bottom)
    }

    /// A glyph of the second face is stretched with the variants and assembly of the second face.
    #[test]
    fn stretching_uses_the_face_of_the_glyph() {
        let composite = composite(ranges(&[0x222B..=0x222B]));
        let reference = primary();
        let integral = composite.shape("\u{222B}", options())[0].glyph_code;
        let theirs = reference.shape("\u{222B}", options())[0].glyph_code;
        assert_eq!(integral >> TAG_SHIFT, 1);
        assert!(composite.is_stretchable(integral, false));
        assert!(!composite.is_stretchable(composite.shape("A", options())[0].glyph_code, false));

        // a size variant: the display size integral has a correction of 540
        let (parts, italic) =
            composite.stretch_with_italic_correction(integral, false, 1500, options());
        assert_eq!(parts.len(), 1);
        assert_eq!((parts[0].1.glyph_code >> TAG_SHIFT, italic), (1, 540));

        // an assembly: parts of the second font, its italic correction, and the size asked for
        for target in [4000, 9000] {
            let (parts, italic) =
                composite.stretch_with_italic_correction(integral, false, target, options());
            let (expected, expected_italic) =
                reference.stretch_with_italic_correction(theirs, false, target, options());
            assert!(parts.len() >= 3, "{target}");
            assert!(
                parts
                    .iter()
                    .all(|(_, glyph)| glyph.glyph_code >> TAG_SHIFT == 1)
            );
            assert_eq!(
                italic, expected_italic,
                "the italic correction of the assembly"
            );
            assert_eq!(italic, 80);
            assert_eq!(parts.len(), expected.len(), "{target}");
            let (top, bottom) = ink(&parts);
            let (expected_top, expected_bottom) = ink(&expected);
            assert!(
                ((top - bottom) - (expected_top - expected_bottom)).abs() <= 4,
                "{target}: {} against {}",
                top - bottom,
                expected_top - expected_bottom
            );
            // and the plain `stretch` is the same parts
            assert_eq!(composite.stretch(integral, false, target, options()), parts);
        }
        // a code of no face gives the .notdef glyph, never nothing
        assert_eq!(
            composite.stretch(0x7F00_0000, false, 100, options()).len(),
            1
        );
    }

    /// The features of a face are applied to its glyphs, and those of the options override them.
    #[test]
    fn a_face_can_have_features() {
        let features = |value| {
            let mut features = FontFeatures::new();
            features.set(*b"ssty", value);
            features
        };
        let code = |face_features: FontFeatures, options: ShapeOptions| {
            let composite = CompositeFont::new(
                vec![
                    FontFace::new(second()).font_features(face_features),
                    FontFace::new(primary()),
                ],
                1,
            )
            .unwrap();
            composite.shape("A", options)[0].glyph_code
        };
        let plain = code(FontFeatures::new(), options());
        let first = code(features(1), options());
        let second = code(features(2), options());
        assert_ne!(plain, first, "ssty 1 gives the alternate");
        assert_ne!(first, second);
        // the options win: ssty off, or on with another value
        let off = ShapeOptions {
            font_features: features(0),
            ..Default::default()
        };
        assert_eq!(code(features(1), off), plain);
        let other = ShapeOptions {
            font_features: features(2),
            ..Default::default()
        };
        assert_eq!(code(features(1), other), second);
        // script level 1 does the same as the feature of the face
        let level_1 = ShapeOptions {
            script_level: 1,
            ..Default::default()
        };
        assert_eq!(code(FontFeatures::new(), level_1), first);
    }

    #[test]
    fn glyph_codes_can_be_decoded_and_asked_for_again() {
        let composite = composite(ranges(&[0x41..=0x41]));
        let (first, second) = (primary(), second());
        let glyphs = composite.shape("AV", options());

        let a = composite.decode_glyph(glyphs[0].glyph_code).unwrap();
        assert_eq!((a.face, a.scale), (0, SCALE));
        assert_eq!(a.glyph, second.shape("A", options())[0].glyph_code);
        assert!(std::ptr::eq(a.font, &composite.faces[0].face.font));
        let v = composite.decode_glyph(glyphs[1].glyph_code).unwrap();
        assert_eq!((v.face, v.scale, v.glyph), (1, 1.0, glyphs[1].glyph_code));
        assert!(std::ptr::eq(
            composite.primary(),
            &composite.faces[1].face.font
        ));
        assert!(
            composite.decode_glyph(0x0500_0000).is_none(),
            "no fifth face"
        );
        assert_eq!(
            composite.decode_glyph(0).unwrap().face,
            1,
            ".notdef is the primary's"
        );

        // asking for a glyph by its code gives what shaping gave
        for options in [
            options(),
            ShapeOptions {
                script_level: 1,
                ..Default::default()
            },
        ] {
            for glyph in composite.shape("AV", options) {
                // a glyph asked for by its code is not from a text, so it has no cluster
                let expected = MathGlyph {
                    cluster: 0,
                    ..glyph
                };
                assert_eq!(composite.glyph(glyph.glyph_code, options), expected);
            }
        }
        assert_eq!(
            composite.glyph(0x0500_0000, options()),
            MathGlyph::default()
        );
        let _ = (first, second);
    }

    /// The ranges of the script and bold script letters that the README gives hold every letter that
    /// `mathvariant="script"` and `"bold-script"` produce, and nothing else but the eleven unassigned
    /// codepoints between them in the block of mathematical alphanumerics.
    #[test]
    fn the_documented_script_ranges_are_those_of_the_script_letters() {
        use crate::typesetting::unicode_math::{Family, convert_character_to_family};
        let documented = ranges(&[
            0x1D49C..=0x1D503,
            0x210A..=0x210B,
            0x2110..=0x2110,
            0x2112..=0x2112,
            0x211B..=0x211B,
            0x212C..=0x212C,
            0x212F..=0x2131,
            0x2133..=0x2134,
        ]);
        let mut letters = std::collections::BTreeSet::new();
        for family in [Family::Script, Family::BoldScript] {
            for c in ('A'..='Z').chain('a'..='z') {
                let letter = convert_character_to_family(c, family);
                assert_ne!(letter, c, "{c:?} in {family:?}");
                assert!(
                    documented.contains(letter),
                    "{letter:?} is not in the ranges"
                );
                letters.insert(letter as u32);
            }
        }
        assert_eq!(letters.len(), 104);
        let unassigned = [
            0x1D49D, 0x1D4A0, 0x1D4A1, 0x1D4A3, 0x1D4A4, 0x1D4A7, 0x1D4A8, 0x1D4AD, 0x1D4BA,
            0x1D4BC, 0x1D4C4,
        ];
        for codepoint in (0x1D49C..=0x1D503).chain(0x210A..=0x2134) {
            let c = char::from_u32(codepoint).unwrap();
            let expected = letters.contains(&codepoint) || unassigned.contains(&codepoint);
            // between U+210A and U+2134 only the eleven script letters are in the ranges
            let in_blocks = codepoint >= 0x1D49C || letters.contains(&codepoint);
            assert_eq!(
                documented.contains(c),
                expected && in_blocks,
                "U+{codepoint:X}"
            );
        }
    }

    /// `size-adjust` of the primary face scales its glyphs and its lengths, but not the em and not the
    /// percentages among the constants.
    #[test]
    fn size_adjust_of_the_primary_face_scales_glyphs_and_lengths_but_not_the_em() {
        let reference = primary();
        let composite =
            CompositeFont::new(vec![FontFace::new(primary()).size_adjust(0.5)], 0).unwrap();
        let a = composite.shape("A", options())[0];
        let expected = reference.shape("A", options())[0];
        assert_eq!(
            a.glyph_code, expected.glyph_code,
            "the codes of the primary face are its own"
        );
        assert_eq!(a.advance_width, expected.advance_width / 2);
        assert_eq!(
            a.extents.width,
            (expected.extents.width as f64 / 2.0).round() as i32
        );
        assert_eq!(composite.units_per_em(), 1000, "the em is not scaled");
        assert_eq!(composite.decode_glyph(a.glyph_code).unwrap().scale, 0.5);

        use MathConstant::*;
        for c in [
            AxisHeight,
            RadicalRuleThickness,
            FractionRuleThickness,
            DisplayOperatorMinHeight,
            SuperscriptShiftUp,
        ] {
            let value = reference.constant(c);
            assert!(value > 0, "{c:?}");
            assert_eq!(
                composite.constant(c),
                (value as f64 / 2.0).round() as i32,
                "{c:?}"
            );
        }
        for c in [
            ScriptPercentScaleDown,
            ScriptScriptPercentScaleDown,
            RadicalDegreeBottomRaisePercent,
        ] {
            assert!(reference.constant(c) > 0, "{c:?}");
            assert_eq!(
                composite.constant(c),
                reference.constant(c),
                "{c:?} is a percentage"
            );
        }
        // the heights of the kerning table are those of a glyph drawn at half the size: a height of 300
        // of the composite is 600 of the face, above the second band of A (the first above 352)
        assert_eq!(
            composite.math_kerning(a.glyph_code, CornerPosition::TopRight, 300),
            -33
        );
    }
}
