//! How the surd and the rule of a square root join, on the STIX Two Math subset of `tests/testfiles/`
//! (see `make-stix-subset.sh`). In MathML Core (3.3.3) the surd is painted so that the top of its ink
//! is at the top edge of the overbar, and the overbar starts at the advance of the surd. The font makes
//! that exact: the top stroke of every surd glyph is as thick as `RadicalRuleThickness` and reaches
//! 31 to 35 units past the advance, under the start of the rule.
#![cfg(feature = "mathml_parser")]

use formel::host::{Fragment, MathConstant, MathFont};
use formel::mathmlparser;
use formel::shaper::Shaper;
use formel::standalone::StyleOptions;

const SUBSET: &[u8] = include_bytes!("testfiles/stixtwo-math-subset.otf");
const HANGING: &[u8] = include_bytes!("testfiles/stixtwo-math-subset-hanging.otf");

/// Where the parts of a laid out `<msqrt>` are, in px (one per font unit), in its own box, y down.
#[derive(Debug)]
struct Radical {
    /// Ink of the surd: top, bottom, right edge, and where its advance ends.
    ink_top: f32,
    ink_bottom: f32,
    ink_right: f32,
    advance_end: f32,
    glyphs: usize,
    /// x of the first and last glyph (the parts of an assembly have the same x).
    glyph_xs: Vec<f32>,
    rule_left: f32,
    rule_top: f32,
    rule_width: f32,
    rule_height: f32,
    /// The radicand: x and width of its box, top and bottom of its box.
    radicand_x: f32,
    radicand_width: f32,
}

/// `<msqrt>` of a spacer `height` px high (and `depth` deep) in `font` in a display or inline formula.
fn radical(font: &Shaper, height: u32, depth: u32, display: bool) -> Radical {
    let xml = format!(
        r#"<math display="{}"><msqrt><mpadded width="900px" height="{height}px" depth="{depth}px"><mi mathvariant="normal">A</mi></mpadded></msqrt></math>"#,
        if display { "block" } else { "inline" }
    );
    let document = mathmlparser::parse(xml.as_bytes()).expect("valid MathML");
    let options = StyleOptions {
        font_size: font.units_per_em() as f32,
        ..Default::default()
    };
    let result = formel::layout(&document, font, options);
    let root = document.root().unwrap();
    let msqrt = document.node(root).children[0];
    let layout = &result.layouts[&msqrt];

    let (mut ink_top, mut ink_bottom, mut ink_right, mut advance_end) =
        (f32::MAX, f32::MIN, f32::MIN, f32::MIN);
    let (mut glyphs, mut glyph_xs) = (0, vec![]);
    let mut rule = None;
    for fragment in &layout.fragments {
        match fragment {
            Fragment::Glyphs {
                origin,
                glyphs: run,
                ..
            } => {
                for positioned in run {
                    let glyph = font.glyph(positioned.glyph, Default::default());
                    let x = origin.x + positioned.offset.x;
                    let y = origin.y + positioned.offset.y; // the baseline of the glyph
                    ink_top = ink_top.min(y - glyph.extents.ascent as f32);
                    ink_bottom = ink_bottom.max(y + glyph.extents.descent as f32);
                    ink_right = ink_right
                        .max(x + (glyph.extents.left_side_bearing + glyph.extents.width) as f32);
                    advance_end = advance_end.max(x + glyph.advance_width as f32);
                    glyphs += 1;
                    glyph_xs.push(x);
                }
            }
            Fragment::Rule(rect) => {
                assert!(rule.replace(*rect).is_none(), "one rule");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    let rule = rule.expect("the rule over the radicand");
    let radicand = &result.layouts[&document.node(msqrt).children[0]];
    Radical {
        ink_top,
        ink_bottom,
        ink_right,
        advance_end,
        glyphs,
        glyph_xs,
        rule_left: rule.origin.x,
        rule_top: rule.origin.y,
        rule_width: rule.size.width,
        rule_height: rule.size.height,
        radicand_x: radicand.location.x,
        radicand_width: radicand.size.width,
    }
}

/// Spacer heights (with a depth of 100) for which the radical is the base glyph, each of the three
/// size variants and an assembly. The target is the height + 100 + the gap (85, or 170 in a display)
/// + the thickness (68); the sizes are 1188, 1855, 2371 and 2892 units high.
const HEIGHTS: [(u32, &str, bool); 7] = [
    (200, "the base glyph", false),
    (1000, "the first size", false),
    (1700, "the second size", false),
    (2300, "the third size", false),
    (2800, "an assembly", true),
    (4000, "an assembly", true),
    (9000, "a tall assembly", true),
];

#[test]
fn the_surd_and_the_rule_join() {
    let font = Shaper::from_static(SUBSET, 0).unwrap();
    let thickness = font.math_constant(MathConstant::RadicalRuleThickness) as f32;
    for display in [false, true] {
        for (height, what, _) in HEIGHTS {
            let r = radical(&font, height, 100, display);
            let case = format!("{what} (spacer {height}, display {display}): {r:?}");

            // the top of the surd's ink is the top edge of the rule, exactly
            assert_eq!(
                r.rule_top, r.ink_top,
                "top of the ink is top of the rule: {case}"
            );
            // the rule is as thick as the font says, which is as thick as the surd's top stroke
            assert_eq!(r.rule_height, thickness, "{case}");
            // the rule starts at the advance of the surd, and the ink reaches 31 to 35 units into it
            assert_eq!(r.rule_left, r.advance_end, "{case}");
            let overlap = r.ink_right - r.rule_left;
            assert!(
                (31.0..=35.0).contains(&overlap),
                "overlap {overlap}: {case}"
            );
            // the rule is over the radicand, and exactly as wide
            assert_eq!(r.radicand_x, r.rule_left, "{case}");
            assert_eq!(r.rule_width, r.radicand_width, "{case}");
        }
    }
}

/// An assembly is one stem: its parts are at the same x.
#[test]
fn the_parts_of_an_assembled_surd_are_in_line() {
    let font = Shaper::from_static(SUBSET, 0).unwrap();
    for display in [false, true] {
        for (height, what, assembled) in HEIGHTS {
            let r = radical(&font, height, 100, display);
            if assembled {
                assert!(r.glyphs >= 3, "{what}: {} glyphs", r.glyphs);
                assert!(
                    r.glyph_xs.windows(2).all(|pair| pair[0] == pair[1]),
                    "{what}: {:?}",
                    r.glyph_xs
                );
            } else {
                assert_eq!(r.glyphs, 1, "{what}");
            }
        }
    }
}

/// The surd covers the radicand and the gap above it, and is no taller than needed. A size variant is
/// the first that is high enough (their ink is 1187, 1854, 2370 and 2891 units high). An assembly is as
/// tall as the target, give or take 1 unit (it is sized by the full advances of its parts, which in
/// STIX are one unit more than the height of their ink), unless the connectors do not allow it: the
/// bottom part ends in a connector of 200 units, so two joints cannot overlap by more than 200 and an
/// assembly of the bottom part, two extenders and the top part (1905 + 2 * 651 + 642 - 3 * 200 = 3249
/// units) is as short as it gets. A target of 3053 or 3138 is below that.
#[test]
fn the_surd_is_as_tall_as_the_radicand_needs() {
    let font = Shaper::from_static(SUBSET, 0).unwrap();
    let thickness = font.math_constant(MathConstant::RadicalRuleThickness);
    for (display, gap) in [
        (false, font.math_constant(MathConstant::RadicalVerticalGap)),
        (
            true,
            font.math_constant(MathConstant::RadicalDisplayStyleVerticalGap),
        ),
    ] {
        for (height, what, assembled) in HEIGHTS {
            let depth = 100;
            let r = radical(&font, height, depth, display);
            let needed = ((height + depth) as i32 + gap + thickness) as f32;
            let surd_height = r.ink_bottom - r.ink_top;
            let case = format!(
                "{what}: height {surd_height} for a target of {needed} (display {display})"
            );
            if assembled && needed < 3249.0 {
                assert_eq!(
                    surd_height, 3248.0,
                    "the shortest assembly with two extenders: {case}"
                );
            } else if assembled {
                assert!((surd_height - needed).abs() <= 1.0, "{case}");
            } else {
                // the size variants are measured by their advance (1188, 1855, 2371, 2892)
                let size = [
                    (1188.0, 1187.0),
                    (1855.0, 1854.0),
                    (2371.0, 2370.0),
                    (2892.0, 2891.0),
                ]
                .into_iter()
                .find(|&(measure, _)| measure >= needed)
                .map(|(_, ink)| ink);
                assert_eq!(Some(surd_height), size, "{case}");
            }
        }
    }
}

/// Fonts that draw the parts of an assembly below their origin join the rule in the same place.
#[test]
fn a_font_with_hanging_parts_joins_the_rule_in_the_same_place() {
    let upright = Shaper::from_static(SUBSET, 0).unwrap();
    let hanging = Shaper::from_static(HANGING, 0).unwrap();
    for (height, what, _) in HEIGHTS {
        let a = radical(&upright, height, 100, false);
        let b = radical(&hanging, height, 100, false);
        assert_eq!(a.rule_top, b.rule_top, "{what}");
        assert_eq!(a.ink_top, b.ink_top, "{what}");
        assert_eq!(a.rule_left, b.rule_left, "{what}");
        // the bottom as well: parts that are misaligned make the surd longer, not the top higher
        assert_eq!(a.ink_bottom, b.ink_bottom, "{what}");
        assert_eq!(a.glyphs, b.glyphs, "{what}");
    }
}
