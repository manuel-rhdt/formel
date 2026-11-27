//! Damaged font tables must give output or an error, never a panic: the shaper reads every table
//! through fallible accessors and treats what it cannot read as absent.
#![cfg(all(feature = "mathml_parser", feature = "svg"))]

use formel::host::{CornerPosition, MathConstant, MathFont, ShapeOptions};
use formel::mathmlparser;
use formel::shaper::Shaper;
use formel::standalone::StyleOptions;
use formel::svg_renderer::{self, Flags};

const FONT: &[u8] = include_bytes!("testfiles/latinmodern-math.otf");

const FORMULA: &str = r#"<math display="block">
  <mrow><mi>f</mi><mo>(</mo><mfrac><msup><mi>x</mi><mn>2</mn></msup><msqrt><mi>y</mi><mo>+</mo><mn>1</mn></msqrt></mfrac><mo>)</mo>
  <mo>=</mo><munderover><mo>&#x2211;</mo><mrow><mi>i</mi><mo>=</mo><mn>0</mn></mrow><mi>&#x221E;</mi></munderover>
  <mo>&#x222B;</mo><mover><mi>a</mi><mo>&#x2192;</mo></mover><mo stretchy="true">&#x23DE;</mo>
  <mo>{</mo><mtable><mtr><mtd><mi>a</mi></mtd></mtr><mtr><mtd><mi>b</mi></mtd></mtr></mtable><mo>}</mo></mrow>
</math>"#;

/// Where the table `tag` lives in the font file: (offset of its directory record, data offset, length).
fn table(data: &[u8], tag: &[u8; 4]) -> (usize, usize, usize) {
    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    for i in 0..num_tables {
        let record = 12 + 16 * i;
        if &data[record..record + 4] == tag {
            let be = |at: usize| u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
            return (record, be(record + 8), be(record + 12));
        }
    }
    panic!("font has no {} table", String::from_utf8_lossy(tag));
}

/// A tiny deterministic generator, so that failures reproduce.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u8 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u8
    }
}

/// Exercises everything the engine asks of a font, plus a complete layout and SVG rendering.
fn exercise(data: Vec<u8>) {
    let Some(font) = Shaper::from_vec(data, 0) else {
        return; // not recognisable as a math font any more: refusing it is fine
    };
    for options in [
        ShapeOptions::default(),
        ShapeOptions {
            script_level: 1,
            ..Default::default()
        },
    ] {
        let glyphs = font.shape("f(x)=∑∫{−}→√", options);
        for glyph in glyphs
            .iter()
            .map(|g| g.glyph_code)
            .chain([0, 1, 40, 100, 3000, 70000])
        {
            font.glyph(glyph, options);
            for horizontal in [false, true] {
                font.is_stretchable(glyph, horizontal);
                for target in [0, 500, 5000, 100_000] {
                    font.stretch(glyph, horizontal, target, options);
                }
            }
            for corner in [
                CornerPosition::TopRight,
                CornerPosition::TopLeft,
                CornerPosition::BottomRight,
                CornerPosition::BottomLeft,
            ] {
                font.math_kerning(glyph, corner, 300);
            }
        }
    }
    font.math_constant(MathConstant::AxisHeight);
    font.math_constant(MathConstant::DisplayOperatorMinHeight);
    font.math_constant(MathConstant::RadicalDegreeBottomRaisePercent);
    let _ = font.take_shape_error();

    let document = mathmlparser::parse(FORMULA.as_bytes()).expect("valid MathML");
    let upem = font.units_per_em().max(1) as f32;
    let typeset = formel::layout(
        &document,
        &font,
        StyleOptions {
            font_size: upem,
            ..Default::default()
        },
    );
    let _ =
        svg_renderer::render_to_string(&document, &typeset.layouts, &font, upem, Flags::default());
}

#[test]
fn the_undamaged_font_works() {
    exercise(FONT.to_vec());
}

#[test]
fn truncated_tables() {
    for tag in [
        b"MATH", b"GSUB", b"GDEF", b"cmap", b"hmtx", b"CFF ", b"head",
    ] {
        let (record, _, length) = table(FONT, tag);
        for new_length in [0, 1, 4, 10, 12, 30, length / 3, length / 2, length - 1] {
            let mut data = FONT.to_vec();
            data[record + 12..record + 16].copy_from_slice(&(new_length as u32).to_be_bytes());
            exercise(data);
        }
    }
}

#[test]
fn garbage_tables() {
    for tag in [b"MATH", b"GSUB", b"GDEF", b"cmap", b"hmtx", b"CFF "] {
        let (_, offset, length) = table(FONT, tag);
        for seed in 0..6u64 {
            let mut random = Lcg(seed + 1);
            let mut data = FONT.to_vec();
            for byte in &mut data[offset..offset + length] {
                *byte = random.next();
            }
            exercise(data);
        }
    }
}

/// Mostly intact tables with a few flipped bytes: offsets and counts that point nowhere or are
/// huge, which is what real corruption looks like.
#[test]
fn corrupted_tables() {
    for tag in [b"MATH", b"GSUB", b"GDEF", b"CFF "] {
        let (_, offset, length) = table(FONT, tag);
        for seed in 0..40u64 {
            let mut random = Lcg(seed * 7919 + 3);
            let mut data = FONT.to_vec();
            for _ in 0..8 {
                // the header and the first records hold the offsets and counts
                let at = offset
                    + (random.next() as usize * 4 + random.next() as usize) % length.min(1200);
                data[at] = random.next();
            }
            exercise(data);
        }
    }
}
