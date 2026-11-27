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
