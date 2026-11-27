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

// ---- glyph assemblies: tolerance for an impossible `MinConnectorOverlap`, bottom alignment ----

fn part(glyph: u32, start: i32, end: i32, full_advance: i32, extender: bool) -> Part {
    Part {
        glyph,
        start_connector_length: start,
        end_connector_length: end,
        full_advance,
        extender,
    }
}

/// The assembly of `(` in Asana Math 0.907, bottom to top. Its `MinConnectorOverlap` is 100, but
/// no connector is longer than 35 units and the extender's are 15, so by the letter of MathML Core
/// the assembly is invalid.
fn asana_0_907_parenthesis() -> Vec<Part> {
    vec![
        part(1, 0, 35, 886, false),
        part(2, 15, 15, 1122, true),
        part(3, 35, 0, 885, false),
    ]
}

#[test]
fn an_impossible_minimum_overlap_fails_without_tolerance() {
    assert_eq!(plan_assembly(&asana_0_907_parenthesis(), 100, 3000), None);
}

#[test]
fn an_impossible_minimum_overlap_is_lowered_to_what_the_connectors_allow() {
    let parts = asana_0_907_parenthesis();
    let plan = plan_assembly_tolerantly(&parts, 100, 3000).expect("an assembly");
    // the connectors of the extender are the shortest: 15 units
    assert_eq!(largest_overlap_the_connectors_allow(&parts), Some(15));
    assert_eq!(plan.connector_overlap, 15.0);
    // and it is as long as asked: 886 + 885 + r * 1122 - 15 per joint
    let length = |r: i32| 886 + 885 + r * 1122 - 15 * (1 + r);
    assert!(length(plan.repeat_count_ext as i32) >= 3000);
    assert!(
        length(plan.repeat_count_ext as i32 - 1) < 3000,
        "no extender more than needed"
    );
    // it is the plan of the same font with the minimum it should have had
    assert_eq!(Some(plan), plan_assembly(&parts, 15, 3000));
}

/// A font that allows its assembly gets exactly the plan it always got.
#[test]
fn a_possible_assembly_is_planned_as_before() {
    // Asana Math 0.962: MinConnectorOverlap 10, connectors from 15 to 35
    let parts = vec![
        part(1, 0, 35, 884, false),
        part(2, 15, 15, 326, true),
        part(3, 35, 0, 884, false),
    ];
    for target in [0, 500, 1768, 1769, 3000, 9000] {
        let strict = plan_assembly(&parts, 10, target);
        assert!(strict.is_some(), "{target}");
        assert_eq!(
            plan_assembly_tolerantly(&parts, 10, target),
            strict,
            "{target}"
        );
    }
}

/// Without any usable overlap (a joint that cannot overlap at all), or without extender that grows
/// the assembly, there is nothing to tolerate.
#[test]
fn nothing_is_tolerated_without_a_usable_overlap_or_a_growing_extender() {
    let no_joint = vec![
        part(1, 0, 0, 500, false),
        part(2, 0, 20, 500, true),
        part(3, 20, 0, 500, false),
    ];
    assert_eq!(plan_assembly_tolerantly(&no_joint, 100, 3000), None);
    let no_extender = vec![part(1, 0, 50, 900, false), part(2, 50, 0, 900, false)];
    assert_eq!(plan_assembly_tolerantly(&no_extender, 100, 3000), None);
    // an extender that does not grow when joined
    let flat = vec![
        part(1, 0, 50, 900, false),
        part(2, 50, 50, 40, true),
        part(3, 50, 0, 900, false),
    ];
    assert_eq!(plan_assembly_tolerantly(&flat, 50, 3000), None);
    assert_eq!(plan_assembly_tolerantly(&[], 50, 3000), None);
}

// MATH header: version (4 bytes), then the offsets of MathConstants, MathGlyphInfo, MathVariants.
// The first field of MathVariants is `minConnectorOverlap`.
fn with_min_connector_overlap(font: &[u8], value: u16) -> Vec<u8> {
    let mut data = font.to_vec();
    let tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let record = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&r| &data[r..r + 4] == b"MATH")
        .expect("a MATH table");
    let be32 = |d: &[u8], at: usize| u32::from_be_bytes(d[at..at + 4].try_into().unwrap()) as usize;
    let math = be32(&data, record + 8);
    let variants = math + u16::from_be_bytes([data[math + 8], data[math + 9]]) as usize;
    data[variants..variants + 2].copy_from_slice(&value.to_be_bytes());
    data
}

const SUBSET: &[u8] = include_bytes!("../../../tests/testfiles/stixtwo-math-subset.otf");
const SUBSET_HANGING: &[u8] =
    include_bytes!("../../../tests/testfiles/stixtwo-math-subset-hanging.otf");

/// The ink of each part of a stretched vertical glyph: (name, bottom, top), y pointing up.
fn ink_of_parts(font: &Shaper, target: i32) -> (Vec<(String, i32, i32)>, i32) {
    let integral = named(font, "uni222B");
    let (parts, correction) =
        font.stretch_with_italic_correction(integral, false, target, level(0));
    let ink = parts
        .iter()
        .map(|(origin, glyph)| {
            let name = font
                .font
                .glyph_name(read_fonts::types::GlyphId::new(glyph.glyph_code))
                .map_or_else(|| glyph.glyph_code.to_string(), |name| name.to_string());
            // y points down in the offsets: the ink bottom is `descent` below the origin
            (
                name,
                -(origin.y + glyph.extents.descent),
                -(origin.y - glyph.extents.ascent),
            )
        })
        .collect();
    (ink, correction)
}

/// The font of the subset has a valid assembly for the integral (`MinConnectorOverlap` 100 against
/// connectors of 600 and 800 units): the parts overlap by what the target needs.
#[test]
fn a_valid_assembly_overlaps_as_the_target_needs() {
    let font = Shaper::from_static(SUBSET, 0).unwrap();
    // uni2321 (1896) + integral.x (1251) + uni2320 (1630): the overlap is the one that makes the
    // assembly exactly 4000 long, as long as the connectors allow it
    let (ink, _) = ink_of_parts(&font, 4000);
    assert_eq!(ink.len(), 3, "{ink:?}");
    let bottoms: Vec<i32> = ink.iter().map(|(_, bottom, _)| *bottom).collect();
    // overlap o: 1896 + 1251 + 1630 - 2 o = 4000, so o = 388.5
    for (bottom, expected) in
        bottoms
            .iter()
            .zip([0.0, 1896.0 - 388.5, 1896.0 + 1251.0 - 2.0 * 388.5])
    {
        assert!((*bottom as f32 - expected).abs() <= 1.0, "{bottoms:?}");
    }
}

/// The same font with `MinConnectorOverlap` 1000, more than any of its connectors: the assembly is
/// still built, with the overlap the connectors allow (600), instead of falling back to the
/// largest size variant.
#[test]
fn an_assembly_with_an_impossible_minimum_overlap_is_still_built() {
    let patched = with_min_connector_overlap(SUBSET, 1000);
    let font = Shaper::from_vec(patched, 0).unwrap();
    assert_eq!(font.min_connector_overlap(), 1000);
    let (ink, correction) = ink_of_parts(&font, 4000);
    let names: Vec<&str> = ink.iter().map(|(name, ..)| name.as_str()).collect();
    assert_eq!(names, ["uni2321", "integral.x", "integral.x", "uni2320"]);
    assert_eq!(
        correction, 80,
        "it is an assembly, with the italic correction of the assembly"
    );
    // consecutive parts overlap by 600: bottoms at 0, 1896 - 600, + (1251 - 600), + (1251 - 600)
    let bottoms: Vec<i32> = ink.iter().map(|(_, bottom, _)| *bottom).collect();
    assert_eq!(bottoms, [0, 1296, 1947, 2598]);
    // at least as tall as asked (it cannot be made shorter than the connectors allow)
    assert!(ink.last().unwrap().2 >= 4000, "{ink:?}");
    // without tolerance the largest variant would have been used: a single glyph
    let strict = plan_assembly(
        &font.assembly_parts(font.font.glyph_by_name("uni222B").unwrap().to_u32(), false),
        1000,
        4000,
    );
    assert_eq!(strict, None);
}

/// Parts drawn below their origin are placed like parts drawn above it (MathML Core 5.3.1: a
/// vertical part is drawn with the bottom of the glyph, not its origin, at the running position).
#[test]
fn vertical_parts_are_aligned_by_their_bottom_edge() {
    let upright = Shaper::from_static(SUBSET, 0).unwrap();
    let hanging = Shaper::from_static(SUBSET_HANGING, 0).unwrap();
    // the origin of the first part is about 1884 units above where the ink starts
    let part = named(&hanging, "uni2321");
    assert_eq!(hanging.glyph(part, level(0)).extents.descent, 1884);
    assert_eq!(upright.glyph(part, level(0)).extents.descent, 0);

    for target in [2400, 3000, 4000, 9000, 20_000] {
        let (a, _) = ink_of_parts(&upright, target);
        let (b, _) = ink_of_parts(&hanging, target);
        assert_eq!(a, b, "the same ink at {target}");
        // and the ink is stacked: each part starts where the previous one ends, less the overlap
        for pair in a.windows(2) {
            let (below, above) = (&pair[0], &pair[1]);
            assert!(above.1 < below.2, "parts overlap: {a:?}");
        }
        assert_eq!(a[0].1, 0, "the assembly starts at the position 0");
    }
}

/// An extender at the end of an assembly is joined to copies of itself, so both of its connectors
/// count even where a non-extender would not use them (the start of the first part).
#[test]
fn the_connectors_of_an_extender_at_the_end_limit_the_tolerated_overlap() {
    // like the bar of Asana Math: extenders only at the start, a short start connector
    let parts = vec![part(1, 10, 30, 500, true), part(2, 40, 0, 500, false)];
    assert_eq!(largest_overlap_the_connectors_allow(&parts), Some(10));
    let plan = plan_assembly_tolerantly(&parts, 100, 3000).expect("an assembly");
    assert!(plan.connector_overlap <= 10.0, "{plan:?}");
    // the same for an extender that comes last
    let parts = vec![part(1, 0, 40, 500, false), part(2, 30, 10, 500, true)];
    assert_eq!(largest_overlap_the_connectors_allow(&parts), Some(10));
}

// ---- the radical: how the glyphs of the surd are made to join the rule ----

/// The points of the outline of a glyph, as (x, y) in font units, from its SVG path data.
fn outline_points(font: &Shaper, glyph: &str) -> Vec<(f32, f32)> {
    let path = font.glyph_path_data(named(font, glyph));
    let numbers: Vec<f32> = path
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .filter(|token| !token.is_empty())
        .map(|token| token.parse().unwrap())
        .collect();
    assert!(
        numbers.len() % 2 == 0 && !numbers.is_empty(),
        "{glyph}: {path}"
    );
    numbers.chunks(2).map(|xy| (xy[0], xy[1])).collect()
}

/// Every glyph of the surd ends in a horizontal stroke, the start of the rule over the radicand, as
/// thick as the font's `RadicalRuleThickness`: it is what the rule joins. The stroke is the right
/// end of the outline, whose top is the highest point of the glyph.
#[test]
fn the_top_stroke_of_every_surd_glyph_is_as_thick_as_the_rule() {
    let font = Shaper::from_static(SUBSET, 0).unwrap();
    let thickness = font.math_constant(MathConstant::RadicalRuleThickness);
    assert_eq!(thickness, 68);
    for glyph in [
        "uni221A",
        "uni221A.s1",
        "uni221A.s2",
        "uni221A.s3",
        "uni221A.t",
    ] {
        let points = outline_points(&font, glyph);
        let right = points.iter().map(|p| p.0).fold(f32::MIN, f32::max);
        let top = points.iter().map(|p| p.1).fold(f32::MIN, f32::max);
        let mut at_the_right_end: Vec<f32> = points
            .iter()
            .filter(|p| p.0 == right)
            .map(|p| p.1)
            .collect();
        at_the_right_end.sort_by(f32::total_cmp);
        assert_eq!(
            at_the_right_end.last(),
            Some(&top),
            "{glyph}: the stroke is at the top"
        );
        assert_eq!(
            top - at_the_right_end[at_the_right_end.len() - 2],
            thickness as f32,
            "{glyph}: thickness of the top stroke"
        );
    }
}

/// The glyphs of the surd reach past their advance, which is where the rule starts: the stroke
/// overlaps the rule instead of ending at it, so that there is no gap (35 units for the sizes, 31
/// for the top part of the assembly).
#[test]
fn the_surd_overlaps_the_start_of_the_rule() {
    let font = Shaper::from_static(SUBSET, 0).unwrap();
    for (glyph, overlap) in [
        ("uni221A", 35),
        ("uni221A.s1", 35),
        ("uni221A.s2", 35),
        ("uni221A.s3", 35),
        ("uni221A.t", 31),
    ] {
        let g = font.glyph(named(&font, glyph), ShapeOptions::default());
        let ink_right = g.extents.left_side_bearing + g.extents.width;
        assert_eq!(ink_right - g.advance_width, overlap, "{glyph}");
    }
}
