//! Composite fonts in a layout and in the SVG, with the STIX Two Math subsets of `tests/testfiles/`:
//! the primary font at 1000 units per em, and the same glyphs at 2048 units per em (without E, F, x
//! and +) as the second face. A face made of the second font must lay out and draw like the primary
//! font alone, give or take a unit of rounding.
#![cfg(all(feature = "mathml_parser", feature = "svg"))]

use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::rc::Rc;

use formel::host::{CompositeFont, FontFace, Fragment, MathFont, NodeLayout, UnicodeRanges};
use formel::mathmlparser;
use formel::shaper::Shaper;
use formel::standalone::{NodeId, StyleOptions};
use formel::svg_renderer::{self, Flags};

const PRIMARY: &[u8] = include_bytes!("testfiles/stixtwo-math-subset.otf");
const SECOND: &[u8] = include_bytes!("testfiles/stixtwo-math-subset-2048.otf");

type Font = CompositeFont<Rc<Shaper>>;

/// The second font for `ranges` (scaled by `size_adjust`), the primary font for the rest.
fn composite(ranges: &[RangeInclusive<u32>], size_adjust: f32) -> Font {
    CompositeFont::new(
        vec![
            FontFace::new(Rc::new(Shaper::from_static(SECOND, 0).unwrap()))
                .unicode_ranges(UnicodeRanges::new(ranges.iter().cloned()))
                .size_adjust(size_adjust),
            FontFace::new(Rc::new(Shaper::from_static(PRIMARY, 0).unwrap())),
        ],
        1,
    )
    .unwrap()
}

/// The primary font alone, as a composite of one face.
fn alone() -> Font {
    CompositeFont::new(
        vec![FontFace::new(Rc::new(
            Shaper::from_static(PRIMARY, 0).unwrap(),
        ))],
        0,
    )
    .unwrap()
}

struct Laid {
    /// By path of child indices from the root: absolute x, width and the glyph codes of the node.
    x: HashMap<Vec<usize>, f32>,
    width: HashMap<Vec<usize>, f32>,
    codes: HashMap<Vec<usize>, Vec<u32>>,
    svg: String,
}

fn lay_out(font: &Font, xml: &str) -> Laid {
    let document = mathmlparser::parse(xml.as_bytes()).unwrap();
    let upem = font.units_per_em() as f32;
    let options = StyleOptions {
        font_size: upem,
        ..Default::default()
    };
    let result = formel::layout(&document, font, options);
    let svg =
        svg_renderer::render_to_string(&document, &result.layouts, font, upem, Flags::default());

    let (mut x, mut width, mut codes) = (HashMap::new(), HashMap::new(), HashMap::new());
    let mut stack: Vec<(NodeId, Vec<usize>, f32)> = vec![(document.root().unwrap(), vec![], 0.0)];
    while let Some((node, path, parent_x)) = stack.pop() {
        let layout: &NodeLayout = &result.layouts[&node];
        let here = parent_x + layout.location.x;
        x.insert(path.clone(), here);
        width.insert(path.clone(), layout.size.width);
        for fragment in &layout.fragments {
            if let Fragment::Glyphs { glyphs, .. } = fragment {
                codes
                    .entry(path.clone())
                    .or_insert_with(Vec::new)
                    .extend(glyphs.iter().map(|glyph| glyph.glyph));
            }
        }
        for (i, &child) in document.node(node).children.iter().enumerate() {
            let mut child_path = path.clone();
            child_path.push(i);
            stack.push((child, child_path, here));
        }
    }
    Laid {
        x,
        width,
        codes,
        svg,
    }
}

impl Laid {
    /// The faces of the glyphs of a node.
    fn tags(&self, path: &[usize]) -> Vec<u32> {
        self.codes[path].iter().map(|code| code >> 24).collect()
    }
}

/// A drawn glyph of the SVG: its scale and the bounding box of its outline in px (y down).
#[derive(Debug)]
struct Drawn {
    scale: f32,
    bounds: [f32; 4],
}

fn drawn_glyphs(svg: &str) -> Vec<Drawn> {
    let mut result = vec![];
    let mut rest = svg;
    while let Some(start) = rest.find("<g transform=\"translate(") {
        rest = &rest[start + 24..];
        let end = rest.find("</g>").unwrap();
        let group = &rest[..end];
        let scale: f32 = group
            .split("scale(")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let path = group
            .split("d=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let numbers: Vec<f32> = path
            .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e'))
            .filter(|token| !token.is_empty())
            .map(|token| token.parse().unwrap())
            .collect();
        let points: Vec<(f32, f32)> = numbers
            .chunks(2)
            .map(|p| (p[0] * scale, p[1] * scale))
            .collect();
        let fold = |f: fn(f32, f32) -> f32, init: f32, pick: fn(&(f32, f32)) -> f32| {
            points.iter().map(pick).fold(init, f)
        };
        result.push(Drawn {
            scale,
            bounds: [
                fold(f32::min, f32::MAX, |p| p.0),
                fold(f32::min, f32::MAX, |p| p.1),
                fold(f32::max, f32::MIN, |p| p.0),
                fold(f32::max, f32::MIN, |p| p.1),
            ],
        });
        rest = &rest[end..];
    }
    result
}

/// The scales of the drawn glyphs, smallest first (the renderer does not draw in document order).
fn sorted_scales(svg: &str) -> Vec<f32> {
    let mut scales: Vec<f32> = drawn_glyphs(svg).iter().map(|glyph| glyph.scale).collect();
    scales.sort_by(f32::total_cmp);
    scales
}

const AVT: &str = r#"<math><mrow><mi mathvariant="normal">A</mi><mi mathvariant="normal">V</mi><mi mathvariant="normal">T</mi></mrow></math>"#;

#[test]
fn a_formula_lays_out_like_the_primary_font_alone() {
    let alone = lay_out(&alone(), AVT);
    // A and T from the second face, V from the primary
    let mixed = lay_out(&composite(&[0x41..=0x41, 0x54..=0x54], 1.0), AVT);
    for path in [[0, 0], [0, 1], [0, 2]] {
        let (a, b) = (alone.x[&path.to_vec()], mixed.x[&path.to_vec()]);
        assert!((a - b).abs() <= 2.0, "{path:?}: {a} against {b}");
    }
    let (a, b) = (alone.width[&vec![0]], mixed.width[&vec![0]]);
    assert!((a - b).abs() <= 2.0, "the row: {a} against {b}");
    assert_eq!(alone.tags(&[0, 0]), [0]);
    assert_eq!(mixed.tags(&[0, 0]), [1], "A comes from the second face");
    assert_eq!(mixed.tags(&[0, 1]), [0], "V from the primary");
    assert_eq!(mixed.tags(&[0, 2]), [1]);
}

#[test]
fn the_svg_draws_each_glyph_with_the_outline_and_scale_of_its_face() {
    let reference = drawn_glyphs(&lay_out(&alone(), AVT).svg);
    let mixed = drawn_glyphs(&lay_out(&composite(&[0x41..=0x41, 0x54..=0x54], 1.0), AVT).svg);
    assert_eq!(reference.len(), 3);
    assert_eq!(mixed.len(), 3);
    assert!(reference.iter().all(|glyph| glyph.scale == 1.0));
    let mut scales: Vec<f32> = mixed.iter().map(|glyph| glyph.scale).collect();
    scales.sort_by(f32::total_cmp);
    assert_eq!(
        scales,
        [0.48828125, 0.48828125, 1.0],
        "1000 / 2048 for A and T, 1 for V"
    );
    // the outlines of the second font are twice as large, and drawn at half the size: the same ink
    for (reference, mixed) in reference.iter().zip(&mixed) {
        for (a, b) in reference.bounds.iter().zip(&mixed.bounds) {
            assert!(
                (a - b).abs() <= 1.5,
                "{:?} against {:?}",
                reference.bounds,
                mixed.bounds
            );
        }
    }
}

#[test]
fn size_adjust_scales_the_layout_and_the_outlines() {
    let laid = lay_out(&composite(&[0x41..=0x41], 0.5), AVT);
    // A is 718 wide in the primary font; half that, and its outline at half the scale
    let advance = laid.x[&vec![0, 1]] - laid.x[&vec![0, 0]];
    assert_eq!(advance, 359.0);
    assert_eq!(sorted_scales(&laid.svg), [0.244140625, 1.0, 1.0]);
}

/// E is in the range of the second face, but the second font has no E: the primary font draws it.
#[test]
fn a_glyph_the_face_lacks_is_drawn_by_the_next_face() {
    let laid = lay_out(
        &composite(&[0x41..=0x7A], 1.0),
        r#"<math><mrow><mi mathvariant="normal">A</mi><mi mathvariant="normal">E</mi></mrow></math>"#,
    );
    assert_eq!(laid.tags(&[0, 0]), [1]);
    assert_eq!(laid.tags(&[0, 1]), [0]);
    assert_eq!(sorted_scales(&laid.svg), [0.48828125, 1.0]);
}

const TALL_INTEGRAL: &str = r#"<math><mrow><msub><mo stretchy="true" symmetric="true">&#x222B;</mo><mi mathvariant="normal">A</mi></msub><mpadded height="3000px" depth="3000px"><mi mathvariant="normal">V</mi></mpadded></mrow></math>"#;

/// An integral of the second face is stretched with the second face's assembly, whose italic
/// correction (80) shifts the subscript left of the advance, as with the primary font.
#[test]
fn a_stretched_operator_of_another_face_is_an_assembly_of_that_face() {
    let reference = lay_out(&alone(), TALL_INTEGRAL);
    let mixed = lay_out(&composite(&[0x222B..=0x222B], 1.0), TALL_INTEGRAL);
    let operator = [0, 0, 0];
    assert!(mixed.codes[&operator.to_vec()].len() >= 3, "an assembly");
    assert!(mixed.tags(&operator).iter().all(|&tag| tag == 1));
    assert!(reference.tags(&operator).iter().all(|&tag| tag == 0));

    // the same width, and the subscript is one correction left of the advance
    let width = mixed.width[&operator.to_vec()];
    assert!((width - reference.width[&operator.to_vec()]).abs() <= 2.0);
    let gap = mixed.x[&vec![0, 0, 1]] - mixed.x[&operator.to_vec()];
    assert_eq!(gap, width - 80.0);
    let reference_gap = reference.x[&vec![0, 0, 1]] - reference.x[&operator.to_vec()];
    assert!((gap - reference_gap).abs() <= 2.0);

    // the parts are drawn at the scale of the second face, A and V at that of the primary
    let drawn = drawn_glyphs(&mixed.svg);
    let scaled = drawn
        .iter()
        .filter(|glyph| glyph.scale == 0.48828125)
        .count();
    assert_eq!(scaled, mixed.codes[&operator.to_vec()].len());
    assert_eq!(drawn.len(), scaled + 2);
}
