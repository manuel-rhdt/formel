//! Tests on `tests/testfiles/stixtwo-math-subset.otf`, a 6 KB subset of STIX Two Math (regenerate
//! it with `tests/testfiles/make-stix-subset.sh`). It has the glyphs A E F T V x + and space, their
//! `ssty` alternates (`A.ssty`, `A.ssty2`, ...) and their MATH kerning. Expected values are the
//! ones stored in the font, so a failure means the font is read differently, not that STIX changed.

use super::*;
use CornerPosition::*;

fn subset() -> Shaper {
    Shaper::from_static(
        include_bytes!("../../../tests/testfiles/stixtwo-math-subset.otf"),
        0,
    )
    .expect("the subset has a MATH table")
}

fn named(font: &Shaper, name: &str) -> u32 {
    font.font
        .glyph_by_name(name)
        .unwrap_or_else(|| panic!("subset has no glyph {name}"))
        .to_u32()
}

fn level(script_level: u8) -> ShapeOptions {
    ShapeOptions {
        script_level,
        ..Default::default()
    }
}

fn features(settings: &str) -> ShapeOptions {
    ShapeOptions {
        font_features: crate::host::parse_font_feature_settings(settings).unwrap(),
        ..Default::default()
    }
}

/// (character, glyph name, advance of the glyph, of `.ssty`, of `.ssty2`)
const SSTY: [(&str, &str, i32, i32, i32); 7] = [
    ("A", "A", 718, 795, 846),
    ("E", "E", 626, 710, 766),
    ("F", "F", 588, 676, 719),
    ("T", "T", 603, 689, 739),
    ("V", "V", 680, 777, 835),
    ("x", "x", 479, 610, 682),
    ("+", "plus", 720, 772, 800),
];

#[test]
fn ssty_picks_the_alternate_of_the_script_level() {
    let font = subset();
    for (text, name, base, first, second) in SSTY {
        for (script_level, glyph, advance) in [
            (0, name.to_string(), base),
            (1, format!("{name}.ssty"), first),
            (2, format!("{name}.ssty2"), second),
        ] {
            let shaped = font.shape(text, level(script_level));
            assert_eq!(shaped.len(), 1, "{text:?}");
            assert_eq!(
                shaped[0].glyph_code,
                named(&font, &glyph),
                "{text:?} at script level {script_level} must be {glyph}"
            );
            assert_eq!(shaped[0].advance_width, advance, "advance of {glyph}");
        }
    }
}

/// The glyph-index path (used for variants and assembly parts) walks GSUB by hand; it must agree
/// with shaping.
#[test]
fn ssty_by_glyph_index_agrees_with_shaping() {
    let font = subset();
    for (text, name, ..) in SSTY {
        let base = named(&font, name);
        for script_level in [0, 1, 2] {
            let shaped = font.shape(text, level(script_level))[0];
            let by_index = font.glyph(base, level(script_level));
            assert_eq!(
                by_index.glyph_code, shaped.glyph_code,
                "{name} level {script_level}"
            );
            assert_eq!(by_index.advance_width, shaped.advance_width);
            assert_eq!(by_index.extents, shaped.extents);
        }
    }
}

/// A feature value counts the alternates from 1; one beyond the last, or 0, substitutes nothing.
#[test]
fn ssty_values_select_alternates_by_index() {
    let font = subset();
    let a = named(&font, "A");
    let glyph = |settings: &str| font.shape("A", features(settings))[0].glyph_code;
    let glyph_by_index = |settings: &str| font.glyph(a, features(settings)).glyph_code;
    for (settings, expected) in [
        ("\"ssty\" 1", "A.ssty"),
        ("\"ssty\" 2", "A.ssty2"),
        ("\"ssty\" 3", "A"),
        ("\"ssty\" 0", "A"),
    ] {
        assert_eq!(glyph(settings), named(&font, expected), "shape, {settings}");
        assert_eq!(
            glyph_by_index(settings),
            named(&font, expected),
            "index, {settings}"
        );
    }
    // there is no third script level either
    assert_eq!(font.shape("A", level(3))[0].glyph_code, a);
    assert_eq!(font.glyph(a, level(3)).glyph_code, a);
}

/// A glyph that `ssty` has no alternates for keeps its glyph at every level.
#[test]
fn ssty_leaves_uncovered_glyphs_alone() {
    let font = subset();
    let space = named(&font, "space");
    for script_level in [0, 1, 2] {
        assert_eq!(font.shape(" ", level(script_level))[0].glyph_code, space);
        assert_eq!(font.glyph(space, level(script_level)).glyph_code, space);
    }
    // an alternate is not itself substituted again
    let alternate = named(&font, "A.ssty");
    assert_eq!(font.glyph(alternate, level(1)).glyph_code, alternate);
}

/// A run is substituted glyph by glyph.
#[test]
fn ssty_applies_to_every_glyph_of_a_run() {
    let font = subset();
    let glyphs: Vec<u32> = font
        .shape("AV x", level(2))
        .iter()
        .map(|g| g.glyph_code)
        .collect();
    let expected: Vec<u32> = ["A.ssty2", "V.ssty2", "space", "x.ssty2"]
        .iter()
        .map(|name| named(&font, name))
        .collect();
    assert_eq!(glyphs, expected);
}

/// (glyph, corner, correction heights, kern values): the bands of the MathKern tables in the font.
/// There is one more kern value than there are heights.
const KERNS: [(&str, CornerPosition, &[i32], &[i32]); 9] = [
    ("A", TopRight, &[252, 352], &[0, -18, -66]),
    ("E", TopRight, &[252], &[32, 31]),
    ("E", BottomRight, &[126], &[24, 32]),
    ("F", BottomRight, &[126, 289], &[-200, -44, 44]),
    ("T", BottomRight, &[285], &[-90, 40]),
    ("T", BottomLeft, &[281], &[-90, 48]),
    ("V", BottomRight, &[126, 280], &[-193, -119, 56]),
    // no correction heights: a single kern for every height
    ("F", TopRight, &[], &[44]),
    ("x", BottomRight, &[], &[35]),
];

/// Below the first height the first kern applies; a height equal to a correction height already
/// belongs to the band above it (`heights[i-1] <= h < heights[i]`, as in HarfBuzz); above the last
/// height the last kern applies.
#[test]
fn corner_kerning_follows_the_bands_of_the_table() {
    let font = subset();
    for (name, corner, heights, values) in KERNS {
        let glyph = named(&font, name);
        let kern = |h: i32| font.math_kerning(glyph, corner, h);
        assert_eq!(kern(i32::MIN), values[0], "{name} {corner:?} far below");
        assert_eq!(
            kern(-1000),
            values[0],
            "{name} {corner:?} below the baseline"
        );
        for (i, &h) in heights.iter().enumerate() {
            assert_eq!(kern(h - 1), values[i], "{name} {corner:?} just below {h}");
            assert_eq!(kern(h), values[i + 1], "{name} {corner:?} at {h}");
            assert_eq!(
                kern(h + 1),
                values[i + 1],
                "{name} {corner:?} just above {h}"
            );
        }
        assert_eq!(
            kern(i32::MAX),
            *values.last().unwrap(),
            "{name} {corner:?} far above"
        );
    }
}

#[test]
fn glyphs_and_corners_without_kerning_have_none() {
    let font = subset();
    let kerned: Vec<(u32, CornerPosition)> = KERNS
        .iter()
        .map(|&(name, corner, ..)| (named(&font, name), corner))
        .collect();
    for name in ["A", "E", "F", "T", "V", "x", "plus", "space", ".notdef"] {
        let glyph = named(&font, name);
        for corner in [TopRight, TopLeft, BottomRight, BottomLeft] {
            if kerned.contains(&(glyph, corner)) {
                continue;
            }
            for h in [-500, 0, 126, 252, 300, 5000] {
                assert_eq!(
                    font.math_kerning(glyph, corner, h),
                    0,
                    "{name} {corner:?} {h}"
                );
            }
        }
    }
    // a glyph id outside the font
    assert_eq!(font.math_kerning(60000, TopRight, 300), 0);
}

/// The alternates have their own kerning entries only if the font has them; the subset's `.ssty`
/// glyphs have none, so script-level glyphs do not inherit the kerning of their base glyph.
#[test]
fn ssty_alternates_do_not_inherit_kerning() {
    let font = subset();
    let alternate = named(&font, "A.ssty");
    for corner in [TopRight, TopLeft, BottomRight, BottomLeft] {
        assert_eq!(font.math_kerning(alternate, corner, 300), 0);
    }
}

/// (character, glyph name, advance, italic correction): the `MathItalicsCorrectionInfo` of the font.
const ITALIC: [(&str, &str, i32, i32); 4] = [
    ("\u{1D453}", "u1D453", 577, 20),  // MATHEMATICAL ITALIC SMALL F
    ("\u{1D447}", "u1D447", 610, 65),  // MATHEMATICAL ITALIC CAPITAL T
    ("\u{1D449}", "u1D449", 640, 100), // MATHEMATICAL ITALIC CAPITAL V
    ("\u{222B}", "uni222B", 684, 230), // INTEGRAL
];

#[test]
fn italic_correction_comes_from_the_table() {
    let font = subset();
    for (text, name, advance, correction) in ITALIC {
        let shaped = font.shape(text, ShapeOptions::default())[0];
        assert_eq!(shaped.glyph_code, named(&font, name));
        assert_eq!(shaped.advance_width, advance, "{name}");
        assert_eq!(shaped.italic_correction, correction, "{name} by shaping");
        let by_index = font.glyph(shaped.glyph_code, ShapeOptions::default());
        assert_eq!(
            by_index.italic_correction, correction,
            "{name} by glyph index"
        );
    }
}

/// Glyphs without an entry have no correction: upright letters, the space, and also the `ssty`
/// alternates of italic glyphs, which the font does not list.
#[test]
fn glyphs_without_an_entry_have_no_italic_correction() {
    let font = subset();
    for name in [
        "A",
        "E",
        "x",
        "plus",
        "space",
        ".notdef",
        "u1D453.ssty",
        "u1D447.ssty2",
        "uni222B.ssty",
    ] {
        let glyph = font.glyph(named(&font, name), ShapeOptions::default());
        assert_eq!(glyph.italic_correction, 0, "{name}");
    }
    assert_eq!(
        font.glyph(60000, ShapeOptions::default()).italic_correction,
        0
    );
}

/// The correction is kept with the substituted glyph: at script level 1 the italic f is
/// `u1D453.ssty`, which has none.
#[test]
fn italic_correction_follows_the_substituted_glyph() {
    let font = subset();
    let base = font.shape("\u{1D453}", level(0))[0];
    let script = font.shape("\u{1D453}", level(1))[0];
    assert_eq!(script.glyph_code, named(&font, "u1D453.ssty"));
    assert_eq!((base.italic_correction, script.italic_correction), (20, 0));
}

/// How the integral stretches in the subset: the base glyph (954 high in the vertical construction
/// table), `uni222B.dsp` (2347) and above that an assembly of `uni2321`, `integral.x` (an extender)
/// and `uni2320`, whose own correction is 80, unlike that of its parts.
#[test]
fn a_stretched_glyph_has_the_italic_correction_of_what_was_chosen() {
    let font = subset();
    let integral = named(&font, "uni222B");
    let dsp = named(&font, "uni222B.dsp");
    let parts_of =
        |target: i32| font.stretch_with_italic_correction(integral, false, target, level(0));

    // (target height, glyphs chosen, correction)
    let (parts, correction) = parts_of(500);
    assert_eq!(
        parts.iter().map(|(_, g)| g.glyph_code).collect::<Vec<_>>(),
        [integral]
    );
    assert_eq!(correction, 230, "the base size integral");

    for target in [955, 1500, 2347] {
        let (parts, correction) = parts_of(target);
        assert_eq!(
            parts.iter().map(|(_, g)| g.glyph_code).collect::<Vec<_>>(),
            [dsp],
            "{target}"
        );
        assert_eq!(correction, 540, "the display size integral at {target}");
    }

    // (target, number of parts): just above the display size the two ends suffice; the extender
    // is repeated for taller targets
    for (target, count) in [(2348, 2), (4000, 3), (20_000, 12)] {
        let (parts, correction) = parts_of(target);
        let glyphs: Vec<u32> = parts.iter().map(|(_, g)| g.glyph_code).collect();
        assert!(
            glyphs.len() == count || (count > 3 && glyphs.len() > 3),
            "parts of an assembly at {target}: {glyphs:?}"
        );
        assert_eq!(
            glyphs[0],
            named(&font, "uni2321"),
            "the bottom part comes first"
        );
        assert_eq!(
            *glyphs.last().unwrap(),
            named(&font, "uni2320"),
            "the top part is last"
        );
        assert_eq!(
            correction, 80,
            "GlyphAssembly.italicsCorrection at {target}"
        );
        // none of the parts has a correction of its own, so only the assembly's value can be it
        assert!(parts.iter().all(|(_, g)| g.italic_correction == 0));
    }
}

#[test]
fn stretching_keeps_returning_the_same_parts() {
    let font = subset();
    let integral = named(&font, "uni222B");
    for target in [0, 500, 1500, 2347, 2348, 9000] {
        let (parts, _) = font.stretch_with_italic_correction(integral, false, target, level(0));
        assert_eq!(
            parts,
            font.stretch(integral, false, target, level(0)),
            "{target}"
        );
    }
}

/// A glyph that does not stretch is returned as it is, with its own correction.
#[test]
fn an_unstretchable_glyph_keeps_its_correction() {
    let font = subset();
    let f = named(&font, "u1D453");
    let (parts, correction) = font.stretch_with_italic_correction(f, false, 5000, level(0));
    assert_eq!(parts.len(), 1);
    assert_eq!(correction, 20);
}

/// A font that does not override `stretch_with_italic_correction` gets the default: the
/// correction of a single glyph, and 0 for an assembly, whose value only the font knows.
#[test]
fn the_default_implementation_knows_single_glyphs_only() {
    struct Plain(Shaper);
    impl MathFont for Plain {
        fn units_per_em(&self) -> u32 {
            self.0.units_per_em()
        }
        fn constant(&self, c: MathConstant) -> i32 {
            self.0.constant(c)
        }
        fn shape(&self, text: &str, options: ShapeOptions) -> Vec<MathGlyph> {
            self.0.shape(text, options)
        }
        fn glyph(&self, glyph: GlyphCode, options: ShapeOptions) -> MathGlyph {
            self.0.glyph(glyph, options)
        }
        fn is_stretchable(&self, glyph: GlyphCode, horizontal: bool) -> bool {
            self.0.is_stretchable(glyph, horizontal)
        }
        fn stretch(
            &self,
            glyph: GlyphCode,
            horizontal: bool,
            target_size: i32,
            options: ShapeOptions,
        ) -> Vec<(Vector<i32>, MathGlyph)> {
            self.0.stretch(glyph, horizontal, target_size, options)
        }
        fn math_kerning(&self, glyph: GlyphCode, corner: CornerPosition, height: i32) -> i32 {
            self.0.math_kerning(glyph, corner, height)
        }
    }
    let font = Plain(subset());
    let integral = named(&font.0, "uni222B");
    assert_eq!(
        font.stretch_with_italic_correction(integral, false, 1500, level(0))
            .1,
        540
    );
    assert_eq!(
        font.stretch_with_italic_correction(integral, false, 9000, level(0))
            .1,
        0
    );
}
