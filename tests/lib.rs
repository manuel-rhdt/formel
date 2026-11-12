#![cfg(feature = "mathml_parser")]

use formel::host::{Fragment, MathElement, MathFont, NodeLayout, Rect};
use formel::mathmlparser;
use formel::standalone::{MathDocument, StyleOptions};

mod util;
use crate::util::TEST_FONT;

/// Lays out `xml` at a font size equal to the font's units per em, so that px are font units.
fn layout_xml(xml: &str, f: impl FnOnce(&MathDocument, &dyn Fn(usize) -> NodeLayout)) {
    TEST_FONT.with(|font| {
        let doc = mathmlparser::parse(xml.as_bytes()).expect("valid MathML");
        let options = StyleOptions {
            font_size: font.units_per_em() as f32,
            ..Default::default()
        };
        let result = formel::layout(&doc, font, options);
        f(&doc, &|id| result.layouts[&id].clone());
    })
}

#[test]
fn mathml_test() {
    let bytes = include_bytes!("testfiles/simple.xml");
    mathmlparser::parse(&bytes[..]).expect("invalid parse");
}

#[test]
fn all_test_files_lay_out() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testfiles");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "xml") {
            continue;
        }
        let xml = std::fs::read_to_string(&path).unwrap();
        layout_xml(&xml, |doc, layout| {
            let root = doc.root().unwrap();
            let root_layout = layout(root);
            assert!(
                root_layout.size.width > 0.0 && root_layout.size.height > 0.0,
                "{:?} has an empty layout",
                path
            );
            // Every node reachable from the root has a layout, except the markers `<none/>` and
            // `<mprescripts/>` of `<mmultiscripts>`, which take no space.
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                if !matches!(
                    doc.node(node).element,
                    MathElement::NoneElement | MathElement::Mprescripts
                ) {
                    let _ = layout(node);
                }
                stack.extend(&doc.node(node).children);
            }
        });
    }
}

#[test]
fn no_scale_division_test() {
    layout_xml("<mi>ab</mi>", |doc, layout| {
        let mi = doc.node(doc.root().unwrap()).children[0];
        let mi_layout = layout(mi);
        let glyphs = match &mi_layout.fragments[..] {
            [Fragment::Glyphs { glyphs, .. }] => glyphs.clone(),
            other => panic!("unexpected fragments {:?}", other),
        };
        assert_eq!(glyphs.len(), 2);
        // the second glyph starts after most of the first one
        assert!(glyphs[1].offset.x > mi_layout.size.width / 2.0 * 0.8);
    })
}

#[test]
fn fraction_centering_test() {
    layout_xml("<mfrac><mn>1</mn><mn>2</mn></mfrac>", |doc, layout| {
        let frac = doc.node(doc.root().unwrap()).children[0];
        let frac_layout = layout(frac);
        let rule: Rect = frac_layout
            .fragments
            .iter()
            .find_map(|f| match f {
                Fragment::Rule(rect) => Some(*rect),
                _ => None,
            })
            .expect("fraction bar");

        for &part in &doc.node(frac).children {
            let part_layout = layout(part);
            let left_space = part_layout.location.x - rule.origin.x;
            let right_space = rule.right() - (part_layout.location.x + part_layout.size.width);
            // allow for differences in side bearings
            assert!(
                (left_space - right_space).abs() <= 30.0,
                "(left, right) = {:?}",
                (left_space, right_space)
            );
        }
    })
}

#[test]
fn table_cells_have_default_padding() {
    layout_xml(
        "<mtable><mtr><mtd><mn>1</mn></mtd></mtr></mtable>",
        |doc, layout| {
            let find = |element| {
                (0..doc.len())
                    .find(|&id| doc.node(id).element == element)
                    .expect("element exists")
            };
            let (cell, number) = (layout(find(MathElement::Mtd)), layout(find(MathElement::Mn)));
            let em = TEST_FONT.with(|font| font.units_per_em() as f32);
            // MathML Core: `mtd { padding: 0.5ex 0.4em }`, with 1ex approximated as 0.5em
            assert!((cell.size.width - number.size.width - 0.8 * em).abs() < 1e-2);
            assert!((cell.size.height - number.size.height - 0.5 * em).abs() < 1e-2);
        },
    );
}

#[test]
fn function_application_respects_an_explicit_rspace() {
    let width = |xml: &str| {
        let mut width = 0.0;
        layout_xml(xml, |doc, layout| width = layout(doc.root().unwrap()).size.width);
        width
    };
    let default = width("<mi>sin</mi><mo>&#x2061;</mo><mi>x</mi>");
    let zero = width(r#"<mi>sin</mi><mo rspace="0">&#x2061;</mo><mi>x</mi>"#);
    let times = width("<mi>sin</mi><mo>&#x2062;</mo><mi>x</mi>");
    assert!(default > zero + 1.0);
    assert!((zero - times).abs() < 1e-2);
}

/// MathML Core (layout of `<mtext>`, shared by `<mi>`, `<mn>` and `<ms>`): only text made of a
/// single glyph has an italic correction. The `n` of `sin` has an entry in the MATH table, but
/// `sin` as a whole must not be treated as slanted.
#[test]
fn only_a_single_glyph_has_an_italic_correction() {
    let width = |xml: &str| {
        let mut width = 0.0;
        layout_xml(xml, |doc, layout| width = layout(doc.root().unwrap()).size.width);
        width
    };
    // An empty `<mspace/>` is not slanted, so it receives the italic correction of what precedes
    // it in the row: appending one shows how much correction a token carries.
    let correction = |token: &str| {
        width(&format!("<mrow>{token}<mspace/></mrow>")) - width(&format!("<mrow>{token}</mrow>"))
    };
    // a lone upright `n` has a correction of 7 units in Latin Modern Math
    assert!((correction("<mi mathvariant=\"normal\">n</mi>") - 7.0).abs() < 1e-2);
    // the same letter at the end of a longer text does not
    assert!(correction("<mi>sin</mi>").abs() < 1e-2);
    assert!(correction("<mi mathvariant=\"normal\">in</mi>").abs() < 1e-2);
    assert!(correction("<mtext>sin</mtext>").abs() < 1e-2);
}

/// Fonts draw the spacing forms of accents (`¯`, `˙`, `´`, ...) lower than the combining accents,
/// which are made to sit over a base. TeX uses the combining ones, so an accent over a base is the
/// combining accent whichever form the MathML names.
#[test]
fn spacing_accents_are_drawn_as_combining_accents() {
    let size = |accent: char| {
        // not stretchy, as LaTeX converters write them: only the glyph is compared
        let xml = format!(
            "<mover accent=\"true\"><mi>x</mi><mo stretchy=\"false\">{accent}</mo></mover>"
        );
        let mut size = None;
        layout_xml(&xml, |doc, layout| {
            let layout = layout(doc.root().unwrap());
            size = Some((layout.size.width, layout.size.height));
        });
        size.unwrap()
    };
    for (spacing, combining) in [
        ('`', '\u{300}'),
        ('\u{B4}', '\u{301}'),
        ('\u{AF}', '\u{304}'),
        ('\u{2D8}', '\u{306}'),
        ('\u{2D9}', '\u{307}'),
        ('\u{A8}', '\u{308}'),
        ('\u{2DA}', '\u{30A}'),
        ('\u{2C7}', '\u{30C}'),
    ] {
        assert_eq!(size(spacing), size(combining), "{spacing:?} vs {combining:?}");
    }
}

/// MathML Core 3.3.1.2: a child of a row is slanted if it is not an embellished operator and has
/// an italic correction. A large operator has a big one (for its scripts), but it must neither
/// push the next child away nor be spared the correction of the letter before it.
#[test]
fn operators_are_never_slanted_in_a_row() {
    let width = |xml: &str| {
        let mut width = 0.0;
        layout_xml(xml, |doc, layout| width = layout(doc.root().unwrap()).size.width);
        width
    };
    let integral = r#"<mo lspace="0" rspace="0">&#x222B;</mo>"#;
    let alone = width(integral);
    // the integral sign has an italic correction of 332 units in Latin Modern Math, but nothing
    // after it receives it
    assert!((width(&format!("<mrow>{integral}<mspace/></mrow>")) - alone).abs() < 1e-2);
    let paren = r#"<mo stretchy="false" lspace="0" rspace="0">(</mo>"#;
    assert!(
        (width(&format!("<mrow>{integral}{paren}</mrow>")) - alone - width(paren)).abs() < 1e-2
    );
    // while the italic correction of the `f` before it (90 units) is applied
    let f = width("<mi>f</mi>");
    assert!((width(&format!("<mrow><mi>f</mi>{integral}</mrow>")) - f - alone - 90.0).abs() < 1e-2);
}

/// MathML Core 3.3.2 (and TeX): the numerator and the denominator are centred by their margin
/// boxes, and the bar is as wide as the wider one. Not by their ink: that would shift the narrower
/// part by the difference of the side bearings and the italic overhang of the two.
#[test]
fn fractions_centre_their_parts_by_advance() {
    layout_xml("<mfrac><mi>a</mi><mi>b</mi></mfrac>", |doc, layout| {
        let frac = doc.node(doc.root().unwrap()).children[0];
        let parts = &doc.node(frac).children;
        let (numerator, denominator) = (layout(parts[0]), layout(parts[1]));
        // in Latin Modern Math the italic `a` is 529 units wide and `b` 429
        assert!((numerator.size.width - 529.0).abs() < 1e-2);
        assert!((denominator.size.width - 429.0).abs() < 1e-2);
        // all of it inside the null delimiter space of 120 units on each side
        assert!((numerator.location.x - 120.0).abs() < 1e-2, "{:?}", numerator.location);
        assert!((denominator.location.x - 170.0).abs() < 1e-2, "{:?}", denominator.location);
        let bar = layout(frac)
            .fragments
            .iter()
            .find_map(|f| match f {
                Fragment::Rule(rect) => Some(*rect),
                _ => None,
            })
            .expect("fraction bar");
        assert!((bar.origin.x - 120.0).abs() < 1e-2 && (bar.size.width - 529.0).abs() < 1e-2, "{bar:?}");
    })
}

/// In `<munderover>` the limits are centred on the base and shifted by half of the italic
/// correction of the base, the one above to the right and the one below to the left (MathML Core
/// 3.4.2.5, and TeX). The scripts are attached one after the other, and the second must not see
/// the first one's box instead of the base: where a script is is the same as with the other
/// script alone.
#[test]
fn limits_of_munderover_are_placed_as_with_one_script() {
    let offsets = |xml: &str| {
        let mut offsets = vec![];
        layout_xml(xml, |doc, layout| {
            let construct = doc.node(doc.root().unwrap()).children[0];
            let children = &doc.node(construct).children;
            offsets = children.iter().map(|&c| layout(c).location.x).collect::<Vec<_>>();
        });
        offsets
    };
    let integral = "<mo>&#x222B;</mo>";
    let [base, sub] = offsets(&format!("<munder>{integral}<mn>0</mn></munder>"))[..] else {
        panic!()
    };
    let [_, sup] = offsets(&format!("<mover>{integral}<mn>1</mn></mover>"))[..] else {
        panic!()
    };
    let both = offsets(&format!("<munderover>{integral}<mn>0</mn><mn>1</mn></munderover>"));
    // the integral sign has a big italic correction, so the two limits are far apart
    assert!(sup - sub > 100.0, "{sub} {sup}");
    assert_eq!(both.len(), 3);
    assert!((both[0] - base).abs() < 1e-2, "{both:?}");
    assert!((both[1] - sub).abs() < 1e-2, "{both:?} against the subscript alone at {sub}");
    assert!((both[2] - sup).abs() < 1e-2, "{both:?} against the superscript alone at {sup}");
}

/// TeX puts `\nulldelimiterspace` (1.2pt, 0.12em at 10pt) on each side of a fraction, as the
/// fraction is `\left. … \right.` with empty delimiters. The size in font units here is that of
/// the 1000-unit em of the test font.
const NULL_DELIMITER_SPACE: f32 = 120.0;

/// Checks the width of the `<mfrac>` of `xml` (the first child of the root) and where its numerator
/// and denominator are, relative to the fraction.
fn check_null_delimiter_space(xml: &str) {
    layout_xml(xml, |doc, layout| {
        let root = doc.root().unwrap();
        let frac = doc.node(root).children[0];
        assert!(matches!(doc.node(frac).element, MathElement::Mfrac));
        let parts = &doc.node(frac).children;
        let widths: Vec<f32> = parts.iter().map(|&p| layout(p).size.width).collect();
        let bar = widths.iter().cloned().fold(0.0, f32::max);

        let frac_layout = layout(frac);
        assert!(
            (frac_layout.size.width - (bar + 2.0 * NULL_DELIMITER_SPACE)).abs() < 0.5,
            "fraction is {} wide, TeX has the bar ({bar}) plus 2 * {NULL_DELIMITER_SPACE}",
            frac_layout.size.width
        );
        // the numerator and the denominator are centred between the null delimiters
        for (&part, width) in parts.iter().zip(widths) {
            let expected = NULL_DELIMITER_SPACE + (bar - width) / 2.0;
            let x = layout(part).location.x;
            assert!((x - expected).abs() < 0.5, "part at x = {x}, expected {expected}");
        }
    });
}

#[test]
fn fraction_has_null_delimiter_space_text() {
    check_null_delimiter_space("<math><mfrac><mi>a</mi><mi>bcd</mi></mfrac></math>");
}

#[test]
fn fraction_has_null_delimiter_space_display() {
    check_null_delimiter_space(
        r#"<math display="block"><mfrac><mi>abc</mi><mi>d</mi></mfrac></math>"#,
    );
}

#[test]
fn fraction_without_bar_has_null_delimiter_space() {
    // \genfrac{}{}{0pt}{}{a}{b} (and \atop) are generalized fractions as well
    check_null_delimiter_space(
        r#"<math><mfrac linethickness="0"><mi>a</mi><mi>bcd</mi></mfrac></math>"#,
    );
}

#[test]
fn fraction_in_a_row_is_spaced_from_its_neighbours() {
    // x\frac{a}{b}: the row is as wide as its children, and the fraction is 2 * 0.12em wider than
    // its bar, so the gap between the `x` and the numerator or denominator is at least 0.12em.
    layout_xml(
        "<math><mrow><mi>x</mi><mfrac><mi>a</mi><mi>b</mi></mfrac><mi>y</mi></mrow></math>",
        |doc, layout| {
            let row = doc.node(doc.root().unwrap()).children[0];
            let kids = &doc.node(row).children;
            let (x, frac, y) = (layout(kids[0]), layout(kids[1]), layout(kids[2]));
            let numerator = doc.node(kids[1]).children[0];
            let denominator = doc.node(kids[1]).children[1];
            let ink_left = frac.location.x
                + layout(numerator).location.x.min(layout(denominator).location.x);
            let gap_left = ink_left - (x.location.x + x.size.width);
            assert!(gap_left >= NULL_DELIMITER_SPACE - 0.5, "gap before the fraction: {gap_left}");
            let ink_right = frac.location.x
                + (layout(numerator).location.x + layout(numerator).size.width)
                    .max(layout(denominator).location.x + layout(denominator).size.width);
            let gap_right = y.location.x - ink_right;
            assert!(gap_right >= NULL_DELIMITER_SPACE - 0.5, "gap after the fraction: {gap_right}");
        },
    );
}
