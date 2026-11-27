//! Tall stretchy operators built from glyph assemblies, on the subsets of STIX Two Math in
//! `tests/testfiles/` (see `make-stix-subset.sh`): fonts whose `MinConnectorOverlap` is larger than
//! any of their connectors (as in Asana Math 0.907), and fonts that draw the parts of an assembly
//! below their origin (as in Asana Math 0.962), must still give a delimiter of the height asked for.
#![cfg(feature = "mathml_parser")]

use formel::host::{Fragment, MathFont, NodeLayout};
use formel::mathmlparser;
use formel::shaper::Shaper;
use formel::standalone::StyleOptions;

const SUBSET: &[u8] = include_bytes!("testfiles/stixtwo-math-subset.otf");
const HANGING: &[u8] = include_bytes!("testfiles/stixtwo-math-subset-hanging.otf");

/// `font` with the `minConnectorOverlap` of its MATH table replaced.
fn with_min_connector_overlap(font: &[u8], value: u16) -> Vec<u8> {
    let mut data = font.to_vec();
    let tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let record = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&r| &data[r..r + 4] == b"MATH")
        .expect("a MATH table");
    let math = u32::from_be_bytes(data[record + 8..record + 12].try_into().unwrap()) as usize;
    let variants = math + u16::from_be_bytes([data[math + 8], data[math + 9]]) as usize;
    data[variants..variants + 2].copy_from_slice(&value.to_be_bytes());
    data
}

/// The stretched integral next to a spacer that is `half_height` px high and as deep: its layout and
/// the number of glyphs it is made of. The root is at `font_size` = one em per font unit.
fn tall_integral(font: &Shaper, half_height: u32) -> (NodeLayout, usize) {
    let xml = format!(
        r#"<math><mrow><mo stretchy="true" symmetric="true">&#x222B;</mo><mpadded height="{half_height}px" depth="{half_height}px"><mi mathvariant="normal">A</mi></mpadded></mrow></math>"#
    );
    let document = mathmlparser::parse(xml.as_bytes()).expect("valid MathML");
    let options = StyleOptions {
        font_size: font.units_per_em() as f32,
        ..Default::default()
    };
    let result = formel::layout(&document, font, options);
    let root = document.root().unwrap();
    let mrow = document.node(root).children[0];
    let integral = document.node(mrow).children[0];
    let layout = result.layouts[&integral].clone();
    let glyphs = layout
        .fragments
        .iter()
        .map(|fragment| match fragment {
            Fragment::Glyphs { glyphs, .. } => glyphs.len(),
            _ => 0,
        })
        .sum();
    (layout, glyphs)
}

/// The operator must be as high as the spacer needs (twice the height, 3000 px, and a bit more for
/// the axis); the largest size variant of the integral is 2347 high.
#[test]
fn a_tall_operator_is_an_assembly_even_with_an_impossible_minimum_overlap() {
    for min_overlap in [100, 1000] {
        let font = Shaper::from_vec(with_min_connector_overlap(SUBSET, min_overlap), 0).unwrap();
        let (layout, glyphs) = tall_integral(&font, 3000);
        assert!(
            glyphs >= 4,
            "an assembly of parts (minimum overlap {min_overlap}): {glyphs}"
        );
        assert!(
            layout.size.height >= 6000.0,
            "the operator covers the spacer (minimum overlap {min_overlap}): {}",
            layout.size.height
        );
    }
}

/// Parts drawn below their origin give the same operator as parts drawn above it.
#[test]
fn an_assembly_of_hanging_parts_has_the_size_of_one_of_upright_parts() {
    let upright = Shaper::from_static(SUBSET, 0).unwrap();
    let hanging = Shaper::from_static(HANGING, 0).unwrap();
    for half_height in [1500, 3000, 6000] {
        let (a, glyphs_a) = tall_integral(&upright, half_height);
        let (b, glyphs_b) = tall_integral(&hanging, half_height);
        assert_eq!(glyphs_a, glyphs_b, "{half_height}");
        assert!(glyphs_a >= 3, "an assembly at {half_height}");
        assert_eq!((a.size, a.baseline), (b.size, b.baseline), "{half_height}");
        assert_eq!(
            (a.ink_left, a.ink_width),
            (b.ink_left, b.ink_width),
            "{half_height}"
        );
    }
}
