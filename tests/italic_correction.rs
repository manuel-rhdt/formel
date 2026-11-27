//! Where the layout puts things that depend on an italic correction, on
//! `tests/testfiles/stixtwo-math-subset.otf` (see `make-stix-subset.sh`). The values are the ones
//! stored in the font; the layout runs at one em per font unit, so px are font units.
//!
//! The rules the engine follows (MathML Core 3.3.1.2 and 3.4.4; for large operators it agrees with
//! the LuaTeX measurements of `tex_oracle`):
//! - a superscript of an ordinary base sits one italic correction to the right of the base's
//!   advance, a subscript at the advance;
//! - a large operator is the other way round: its superscript sits at the advance and its
//!   subscript one italic correction to the *left* of it, so the two stay one correction apart;
//! - in a row, the correction of a slanted child is added before the next child unless that one is
//!   slanted too, and a large operator's correction is never added.
#![cfg(feature = "mathml_parser")]

use formel::host::{Fragment, MathFont};
use formel::mathmlparser;
use formel::shaper::Shaper;
use formel::standalone::{NodeId, StyleOptions};

const FONT: &[u8] = include_bytes!("testfiles/stixtwo-math-subset.otf");

/// Absolute x of every node, by path of child indices from the root, plus the font.
struct Laid {
    font: Shaper,
    xs: std::collections::HashMap<Vec<usize>, f32>,
    /// The first glyph of the node's own run.
    glyphs: std::collections::HashMap<Vec<usize>, u32>,
    /// Width of the border box, and the number of glyphs of the node's own run.
    widths: std::collections::HashMap<Vec<usize>, f32>,
    run_lengths: std::collections::HashMap<Vec<usize>, usize>,
}

fn lay_out(xml: &str) -> Laid {
    let font = Shaper::from_static(FONT, 0).expect("the subset has a MATH table");
    let document = mathmlparser::parse(xml.as_bytes()).expect("valid MathML");
    let options = StyleOptions {
        font_size: font.units_per_em() as f32,
        ..Default::default()
    };
    let result = formel::layout(&document, &font, options);

    let mut xs = std::collections::HashMap::new();
    let mut glyphs = std::collections::HashMap::new();
    let mut widths = std::collections::HashMap::new();
    let mut run_lengths = std::collections::HashMap::new();
    let root = document.root().expect("a root");
    let mut stack: Vec<(NodeId, Vec<usize>, f32)> = vec![(root, vec![], 0.0)];
    while let Some((node, path, parent_x)) = stack.pop() {
        let layout = &result.layouts[&node];
        let x = parent_x + layout.location.x;
        xs.insert(path.clone(), x);
        widths.insert(path.clone(), layout.size.width);
        for fragment in &layout.fragments {
            if let Fragment::Glyphs { glyphs: run, .. } = fragment {
                if let Some(first) = run.first() {
                    glyphs.entry(path.clone()).or_insert(first.glyph);
                }
                *run_lengths.entry(path.clone()).or_insert(0) += run.len();
            }
        }
        for (i, &child) in document.node(node).children.iter().enumerate() {
            let mut child_path = path.clone();
            child_path.push(i);
            stack.push((child, child_path, x));
        }
    }
    Laid {
        font,
        xs,
        glyphs,
        widths,
        run_lengths,
    }
}

impl Laid {
    fn x(&self, path: &[usize]) -> f32 {
        *self
            .xs
            .get(path)
            .unwrap_or_else(|| panic!("no node at {path:?}"))
    }

    fn width(&self, path: &[usize]) -> f32 {
        *self
            .widths
            .get(path)
            .unwrap_or_else(|| panic!("no node at {path:?}"))
    }

    /// x of the middle of the node's border box.
    fn center(&self, path: &[usize]) -> f32 {
        self.x(path) + self.width(path) / 2.0
    }

    fn glyph(&self, path: &[usize]) -> formel::host::MathGlyph {
        let code = *self
            .glyphs
            .get(path)
            .unwrap_or_else(|| panic!("no glyph at {path:?}"));
        self.font.glyph(code, Default::default())
    }
}

const A: &str = r#"<mi mathvariant="normal">A</mi>"#; // upright: no italic correction, no kerning

/// A superscript is placed at the advance of its base plus the base's italic correction (and the
/// base's top-right kern, which the font has for T and not for f or V).
#[test]
fn superscripts_move_right_by_the_italic_correction() {
    // base: f (advance 577, correction 20, no kern), V (640, 100, kern 0), T (610, 65, kern 30)
    for (base, advance, correction, kern) in [
        ("f", 577.0, 20.0, 0.0),
        ("V", 640.0, 100.0, 0.0),
        ("T", 610.0, 65.0, 30.0),
    ] {
        let laid = lay_out(&format!("<math><msup><mi>{base}</mi>{A}</msup></math>"));
        let gap = laid.x(&[0, 1]) - laid.x(&[0, 0]);
        assert_eq!(gap, advance + correction + kern, "superscript of {base}");
    }
}

/// An upright base has no correction: the gap is the advance alone. (Upright V has neither an
/// italic correction nor top-right kerning in the font; A would be kerned by 66 at this height.)
#[test]
fn superscripts_of_upright_bases_get_no_correction() {
    let laid = lay_out(&format!(
        r#"<math><msup><mi mathvariant="normal">V</mi>{A}</msup></math>"#
    ));
    assert_eq!(laid.x(&[0, 1]) - laid.x(&[0, 0]), 680.0, "the advance of V");
}

/// The italic correction of a subscript's base is not used for a subscript: the bottom right of
/// an italic letter is not where the slant puts its ink.
#[test]
fn subscripts_of_italic_letters_ignore_the_italic_correction() {
    // f has kerning at the bottom right (heights 156, 283; kerns -199, -148, 16), which depends on
    // the subscript's height, so only the bounds are checked: it never adds the correction.
    let laid = lay_out(&format!("<math><msub><mi>f</mi>{A}</msub></math>"));
    let gap = laid.x(&[0, 1]) - laid.x(&[0, 0]);
    assert!(
        (577.0 - 199.0..=577.0 + 16.0).contains(&gap),
        "gap {gap} is advance + kern"
    );
}

/// A large operator's scripts: the subscript sits one italic correction left of the advance, the
/// superscript at the advance. The integral sign is slanted, so its subscript tucks under it.
#[test]
fn scripts_of_large_operators_are_one_italic_correction_apart() {
    let integral = |scripts: &str| {
        let laid = lay_out(&format!("<math>{scripts}</math>"));
        let glyph = laid.glyph(&[0, 0]);
        let gap = laid.x(&[0, 1]) - laid.x(&[0, 0]);
        (glyph, gap)
    };

    let (glyph, gap) = integral(&format!("<msub><mo>\u{222B}</mo>{A}</msub>"));
    // the integral, or its display variant: either way a slanted glyph with a big correction
    assert!(glyph.italic_correction >= 230, "{glyph:?}");
    assert_eq!(
        gap,
        (glyph.advance_width - glyph.italic_correction) as f32,
        "subscript"
    );

    let (glyph, gap) = integral(&format!("<msup><mo>\u{222B}</mo>{A}</msup>"));
    assert_eq!(gap, glyph.advance_width as f32, "superscript");
}

/// In a row, the correction of a slanted child separates it from an upright one...
#[test]
fn a_row_adds_the_italic_correction_before_an_upright_child() {
    let laid = lay_out(&format!("<math><mrow><mi>f</mi>{A}</mrow></math>"));
    assert_eq!(laid.x(&[0, 1]) - laid.x(&[0, 0]), 577.0 + 20.0);
}

/// ... but not from another slanted one, which leans the same way.
#[test]
fn a_row_adds_nothing_between_two_slanted_children() {
    for (first, advance) in [("f", 577.0), ("V", 640.0)] {
        let laid = lay_out(&format!(
            "<math><mrow><mi>{first}</mi><mi>T</mi></mrow></math>"
        ));
        assert_eq!(laid.x(&[0, 1]) - laid.x(&[0, 0]), advance, "{first} then T");
    }
}

/// An operator is never slanted, so it does get the correction of what precedes it.
#[test]
fn a_row_adds_the_italic_correction_before_an_operator() {
    let laid = lay_out(r#"<math><mrow><mi>f</mi><mo lspace="0" rspace="0">+</mo></mrow></math>"#);
    assert_eq!(laid.x(&[0, 1]) - laid.x(&[0, 0]), 577.0 + 20.0);
}

/// The correction of a large operator is for its scripts, not for what follows it.
#[test]
fn a_row_does_not_add_the_correction_of_a_large_operator() {
    let laid = lay_out(&format!(
        r#"<math><mrow><mo lspace="0" rspace="0">&#x222B;</mo>{A}</mrow></math>"#
    ));
    let integral = laid.glyph(&[0, 0]);
    assert!(integral.italic_correction >= 230);
    assert_eq!(
        laid.x(&[0, 1]) - laid.x(&[0, 0]),
        integral.advance_width as f32
    );
}

/// The correction a row passes on is that of its last child (the row is slanted if it ends in a
/// slanted glyph), so a superscript of `(A f)` sits after `f`'s correction.
///
/// This counts the correction once. Read literally, MathML Core (Candidate Recommendation of
/// 24 June 2025, 3.3.1) counts it twice: the row's inline size ends with "increment inline-offset
/// by previous-italic-correction", the row's italic correction is that same value, and `msup`
/// adds the base's correction to the base's inline size. That looks like an oversight in the
/// text (TeX adds it once); this test pins what the engine does, not the literal reading.
#[test]
fn a_row_passes_on_the_correction_of_its_last_child() {
    let laid = lay_out(&format!(
        "<math><msup><mrow>{A}<mi>f</mi></mrow>{A}</msup></math>"
    ));
    // the base row is A (718) then f (577), then f's correction
    assert_eq!(laid.x(&[0, 1]) - laid.x(&[0, 0]), 718.0 + 577.0 + 20.0);
}

const INTEGRAL: &str = "\u{222B}";

/// The limits of a large operator follow its slant: the one above is centred half a correction to
/// the right of the base's centre, the one below half a correction to the left (MathML Core 3.4.2,
/// `LargeOpItalicCorrection`), the same in the cases where the script is narrower than the base
/// and where the base has to move because the script is wider.
#[test]
fn limits_of_large_operators_are_shifted_by_half_the_italic_correction() {
    for display in ["inline", "block"] {
        for (element, script, over, under) in [
            ("munder", "A", false, true),
            ("mover", "A", true, false),
            ("munder", "AAAAAAAAAA", false, true), // wider than the integral
            ("mover", "AAAAAAAAAA", true, false),
        ] {
            let xml = format!(
                r#"<math display="{display}"><{element}><mo>{INTEGRAL}</mo><mi mathvariant="normal">{script}</mi></{element}></math>"#
            );
            let laid = lay_out(&xml);
            let integral = laid.glyph(&[0, 0]);
            // 230 at the text size, 540 for the display size glyph
            let expected = if display == "block" { 540 } else { 230 };
            assert_eq!(integral.italic_correction, expected, "{display} {element}");
            let half = integral.italic_correction as f32 / 2.0;
            let shift = laid.center(&[0, 1]) - laid.center(&[0, 0]);
            let want = if over {
                half
            } else if under {
                -half
            } else {
                unreachable!()
            };
            assert_eq!(shift, want, "{display} {element} with {script}");
        }
    }
}

/// With both limits each gets its own half, from the correction of the base itself.
#[test]
fn both_limits_are_shifted_in_opposite_directions() {
    for (display, correction) in [("inline", 230.0), ("block", 540.0)] {
        let xml = format!(
            r#"<math display="{display}"><munderover><mo>{INTEGRAL}</mo><mi mathvariant="normal">A</mi><mi mathvariant="normal">E</mi></munderover></math>"#
        );
        let laid = lay_out(&xml);
        let base = laid.center(&[0, 0]);
        assert_eq!(
            laid.center(&[0, 1]) - base,
            -correction / 2.0,
            "{display} under"
        );
        assert_eq!(
            laid.center(&[0, 2]) - base,
            correction / 2.0,
            "{display} over"
        );
    }
}

/// An operator that is not a large operator, even with a correction of its own, centres its
/// limits: only `largeop` bases lean (the spec's `LargeOpItalicCorrection` is 0 for the others).
#[test]
fn limits_of_ordinary_slanted_bases_are_centred() {
    for element in ["munder", "mover"] {
        let laid = lay_out(&format!(
            "<math><{element}><mi>f</mi>{A}</{element}></math>"
        ));
        assert_eq!(
            laid.glyph(&[0, 0]).italic_correction,
            20,
            "the italic f is slanted"
        );
        assert_eq!(laid.center(&[0, 1]), laid.center(&[0, 0]), "{element}");
    }
}

/// A stretchy operator that the font builds from an assembly has the italic correction of the
/// assembly (80), not that of the glyph it started from (230), nor that of its parts (none): the
/// subscript of a large operator sits one correction left of its advance.
#[test]
fn an_assembled_operator_has_the_italic_correction_of_the_assembly() {
    // the row's tall spacer makes the integral stretch to more than the display size glyph
    let tall =
        r#"<mpadded height="3000px" depth="3000px"><mi mathvariant="normal">A</mi></mpadded>"#;
    let xml = format!(
        r#"<math><mrow><msub><mo stretchy="true" symmetric="true">{INTEGRAL}</mo>{A}</msub>{tall}</mrow></math>"#
    );
    let laid = lay_out(&xml);
    let operator = [0, 0, 0];
    assert!(
        laid.run_lengths[&operator.to_vec()] >= 3,
        "the integral is an assembly of parts"
    );
    let gap = laid.x(&[0, 0, 1]) - laid.x(&operator);
    assert_eq!(
        gap,
        laid.width(&operator) - 80.0,
        "advance of the assembly - its correction"
    );
}

/// The same for the sizes the font offers as glyphs: the display size integral has its own
/// correction (540), found through the stretching algorithm and not only by shaping.
#[test]
fn a_stretched_variant_has_the_italic_correction_of_the_variant() {
    // tall enough to need the display size glyph (2347) but not an assembly
    let tall = r#"<mpadded height="800px" depth="800px"><mi mathvariant="normal">A</mi></mpadded>"#;
    let xml = format!(
        r#"<math><mrow><msub><mo stretchy="true" symmetric="true">{INTEGRAL}</mo>{A}</msub>{tall}</mrow></math>"#
    );
    let laid = lay_out(&xml);
    let operator = [0, 0, 0];
    assert_eq!(laid.run_lengths[&operator.to_vec()], 1, "a single glyph");
    let glyph = laid.glyph(&operator);
    assert_eq!(glyph.italic_correction, 540, "the display size integral");
    let gap = laid.x(&[0, 0, 1]) - laid.x(&operator);
    assert_eq!(gap, laid.width(&operator) - 540.0);
}
