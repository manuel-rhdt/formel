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
    let root = document.root().expect("a root");
    let mut stack: Vec<(NodeId, Vec<usize>, f32)> = vec![(root, vec![], 0.0)];
    while let Some((node, path, parent_x)) = stack.pop() {
        let layout = &result.layouts[&node];
        let x = parent_x + layout.location.x;
        xs.insert(path.clone(), x);
        for fragment in &layout.fragments {
            if let Fragment::Glyphs { glyphs: run, .. } = fragment {
                if let Some(first) = run.first() {
                    glyphs.entry(path.clone()).or_insert(first.glyph);
                }
            }
        }
        for (i, &child) in document.node(node).children.iter().enumerate() {
            let mut child_path = path.clone();
            child_path.push(i);
            stack.push((child, child_path, x));
        }
    }
    Laid { font, xs, glyphs }
}

impl Laid {
    fn x(&self, path: &[usize]) -> f32 {
        *self
            .xs
            .get(path)
            .unwrap_or_else(|| panic!("no node at {path:?}"))
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
/// slanted glyph), so a superscript of `(a f)` sits after `f`'s correction.
#[test]
fn a_row_passes_on_the_correction_of_its_last_child() {
    let laid = lay_out(&format!(
        "<math><msup><mrow>{A}<mi>f</mi></mrow>{A}</msup></math>"
    ));
    // the base row is A (718) then f (577), then f's correction
    assert_eq!(laid.x(&[0, 1]) - laid.x(&[0, 0]), 718.0 + 577.0 + 20.0);
}
