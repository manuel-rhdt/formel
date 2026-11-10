#![cfg(feature = "mathml_parser")]

use math_render::host::{Fragment, MathElement, MathFont, NodeLayout, Rect};
use math_render::mathmlparser;
use math_render::standalone::{MathDocument, StyleOptions};

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
        let result = math_render::layout(&doc, font, options);
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
