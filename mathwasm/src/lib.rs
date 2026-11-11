//! WebAssembly bindings for `formel`.
//!
//! ```js
//! import init, { MathRenderer } from "./pkg/mathwasm.js";
//! await init();
//! const font = new Uint8Array(await (await fetch("font.otf")).arrayBuffer());
//! const renderer = new MathRenderer(font);
//! container.innerHTML = renderer.render_svg("<math><mi>x</mi></math>");
//! ```

use formel::mathmlparser;
use formel::shaper::Shaper;
use formel::standalone::StyleOptions;
use formel::svg_renderer;
use wasm_bindgen::prelude::*;

/// The result of [`MathRenderer::render`]: the SVG and where the baseline is in it.
///
/// All lengths are in font design units (see [`MathRenderer::units_per_em`]) and describe the
/// SVG's `viewBox`, margin included. Dividing by `units_per_em` gives em of the font, which is
/// what you need to size an inline image next to text and align it on the baseline.
#[wasm_bindgen]
pub struct Rendering {
    svg: String,
    alt: String,
    width: f32,
    height: f32,
    baseline: f32,
}

#[wasm_bindgen]
impl Rendering {
    /// The SVG document.
    #[wasm_bindgen(getter)]
    pub fn svg(&self) -> String {
        self.svg.clone()
    }

    /// The formula's accessible name: the author's own text when the MathML carries one, otherwise
    /// its text content. Empty when the expression has no text at all.
    #[wasm_bindgen(getter)]
    pub fn alt(&self) -> String {
        self.alt.clone()
    }

    /// Width of the SVG's `viewBox`.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Height of the SVG's `viewBox`.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Distance from the top of the SVG's `viewBox` to the formula's alphabetic baseline.
    #[wasm_bindgen(getter)]
    pub fn baseline(&self) -> f32 {
        self.baseline
    }
}

/// A math font that can render MathML to SVG.
///
/// The font bytes are copied once on construction and reused for every call.
#[wasm_bindgen]
pub struct MathRenderer {
    // Leaked so that the `Shaper` can borrow it for `'static`; a renderer lives for as long as
    // the page uses it, and `free()` on the JS side drops only the handle.
    shaper: Shaper<'static>,
}

#[wasm_bindgen]
impl MathRenderer {
    /// Creates a renderer from the bytes of an OpenType font with a `MATH` table.
    #[wasm_bindgen(constructor)]
    pub fn new(font: &[u8]) -> Result<MathRenderer, JsError> {
        let data: &'static [u8] = Box::leak(font.to_vec().into_boxed_slice());
        let shaper = Shaper::from_slice(data, 0)
            .ok_or_else(|| JsError::new("not a valid font with an OpenType MATH table"))?;
        Ok(MathRenderer { shaper })
    }

    /// The font's design units per em. The SVG is sized in these units.
    pub fn units_per_em(&self) -> u32 {
        use formel::host::MathFont;
        self.shaper.units_per_em()
    }

    /// Renders a MathML string (a `<math>` element or a fragment with one root) to an SVG
    /// string. The SVG is sized in font design units, so scale it with CSS (`height: 2em`).
    pub fn render_svg(&self, mathml: &str) -> Result<String, JsError> {
        Ok(self.render_inner(mathml, false)?.svg)
    }

    /// Like [`render_svg`](Self::render_svg) but also outlines the box of every element.
    pub fn render_svg_debug(&self, mathml: &str) -> Result<String, JsError> {
        Ok(self.render_inner(mathml, true)?.svg)
    }

    /// Renders MathML to SVG together with its size and baseline position.
    ///
    /// With `debug` set, the box of every element is outlined. Call `free()` on the result
    /// after reading it.
    pub fn render(&self, mathml: &str, debug: bool) -> Result<Rendering, JsError> {
        self.render_inner(mathml, debug)
    }
}

impl MathRenderer {
    fn render_inner(&self, mathml: &str, debug: bool) -> Result<Rendering, JsError> {
        use formel::host::MathFont;

        let document = mathmlparser::parse(mathml.as_bytes())
            .map_err(|e| JsError::new(&format!("could not parse MathML: {e}")))?;
        if document.root().is_none() {
            return Err(JsError::new("the MathML document is empty"));
        }

        // A font size of one em per font unit makes px equal font units. As in MathML Core
        // (`math { math-style: compact }`), `<math>` is inline unless it has `display="block"`.
        let units_per_em = self.shaper.units_per_em() as f32;
        let options = StyleOptions {
            font_size: units_per_em,
            math_style: formel::host::MathStyle::Compact,
        };
        let typeset = formel::layout(&document, &self.shaper, options);
        let flags = svg_renderer::Flags {
            show_logical_bounds: debug,
            ..Default::default()
        };
        let root = document.root().expect("checked above");
        let root_layout = &typeset.layouts[&root];
        let margin = svg_renderer::MARGIN;
        let alt = alt_text(mathml);
        let svg = svg_renderer::render_to_string_with(
            &document,
            &typeset.layouts,
            self.shaper.face(),
            units_per_em,
            flags,
            svg_renderer::A11y { title: &alt },
        );
        Ok(Rendering {
            svg,
            alt,
            width: root_layout.size.width + 2.0 * margin,
            height: root_layout.size.height + 2.0 * margin,
            baseline: root_layout.baseline + margin,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn renderer() -> MathRenderer {
        MathRenderer::new(include_bytes!("../../tests/testfiles/latinmodern-math.otf"))
            .unwrap_or_else(|_| panic!("test font must load"))
    }

    #[test]
    fn renders_svg() {
        let svg = renderer()
            .render_svg("<math><mfrac><mi>x</mi><mn>2</mn></mfrac></math>")
            .unwrap_or_else(|_| panic!("render failed"));
        assert!(svg.starts_with("<svg"), "{svg}");
        assert!(svg.contains("<path"), "glyph outlines expected");
        assert!(svg.contains("<rect"), "fraction rule expected");
    }

    #[test]
    fn alt_text_prefers_the_author() {
        assert_eq!(
            alt_text(r#"<math alttext="x squared"><msup><mi>x</mi><mn>2</mn></msup></math>"#),
            "x squared"
        );
        assert_eq!(
            alt_text(
                r#"<math><semantics><msup><mi>x</mi><mn>2</mn></msup><annotation encoding="text/plain">x squared</annotation></semantics></math>"#
            ),
            "x squared"
        );
        assert_eq!(alt_text("<math><mi>x</mi></math>"), "x");
        // The fallback is deliberately crude — concatenated text, no operator names. Anything that
        // cares about the wording supplies an annotation instead.
        assert_eq!(
            alt_text("<math><mi>x</mi><mo>+</mo><mn>1</mn></math>"),
            "x+1"
        );
        assert_eq!(alt_text(""), "");
    }

    #[test]
    fn rendering_carries_an_accessible_name() {
        let out = renderer()
            .render(
                r#"<math alttext="x over two"><mfrac><mi>x</mi><mn>2</mn></mfrac></math>"#,
                false,
            )
            .unwrap_or_else(|_| panic!("render failed"));
        assert_eq!(out.alt(), "x over two");
        let svg = out.svg();
        assert!(svg.contains(r#"role="img""#), "{svg}");
        assert!(svg.contains("<title>x over two</title>"), "{svg}");
    }

    #[test]
    fn every_demo_font_loads_and_renders() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/fonts");
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if !matches!(path.extension().and_then(|e| e.to_str()), Some("otf" | "ttf")) {
                continue;
            }
            let name = path.display().to_string();
            let font = std::fs::read(&path).unwrap();
            let renderer = MathRenderer::new(&font).unwrap_or_else(|_| panic!("{name}: no MATH table"));
            let svg = convert_latex(r"\sum_{i=0}^n \frac{x_i}{\sqrt{2}} \left(a+b\right)", true)
                .and_then(|m| renderer.render_svg(&m).map_err(|_| "render failed".to_string()))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(svg.contains("<path"), "{name}");
            count += 1;
        }
        assert!(count >= 5, "expected the demo fonts, found {count}");
    }

    #[test]
    fn rendering_reports_size_and_baseline() {
        let r = renderer();
        let result = r
            .render("<math><mfrac><mi>x</mi><mn>2</mn></mfrac></math>", false)
            .unwrap_or_else(|_| panic!());
        let svg = result.svg();
        let (w, h) = (result.width(), result.height());
        assert!(svg.contains(&format!("{} {}", w, h)), "viewBox should match: {svg}");
        // the baseline is inside the box, and a fraction has stuff below it
        assert!(result.baseline() > 0.0 && result.baseline() < h);
        // a lone `x` sits on the baseline, so there is hardly anything below it
        let x = r.render("<math><mi>x</mi></math>", false).unwrap_or_else(|_| panic!());
        assert!(x.height() - x.baseline() < 0.3 * r.units_per_em() as f32);
    }

    #[test]
    fn math_is_inline_unless_display_block() {
        let r = renderer();
        let size = |mathml: &str| {
            let out = r.render(mathml, false).unwrap_or_else(|_| panic!("{mathml}"));
            (out.width(), out.height())
        };
        let sum = "<munderover><mo>∑</mo><mi>a</mi><mi>b</mi></munderover>";
        let inline = size(&format!("<math>{sum}</math>"));
        let block = size(&format!("<math display=\"block\">{sum}</math>"));
        assert_eq!(inline, size(&format!("<math display=\"inline\">{sum}</math>")));
        // inline: small operator with limits at the side; display: large with limits stacked
        assert!(inline.1 < block.1 && inline.0 > block.0, "{inline:?} {block:?}");
        // and the LaTeX switch maps onto it (math-core sets the operator spacing, so compare
        // only the heights)
        let latex = |display| size(&convert_latex(r"\sum_a^b", display).unwrap()).1;
        assert_eq!(latex(false), inline.1);
        assert_eq!(latex(true), block.1);
    }

    #[test]
    fn debug_output_has_boxes() {
        let r = renderer();
        let mathml = "<math><mi>x</mi></math>";
        let plain = r.render_svg(mathml).unwrap_or_else(|_| panic!());
        let debug = r.render_svg_debug(mathml).unwrap_or_else(|_| panic!());
        assert!(debug.len() > plain.len());
    }

    #[test]
    fn renders_the_demo_formula() {
        let html = include_str!("../../web/index.html");
        let start = html.find("<math").expect("demo has MathML");
        let end = html.find("</math>").expect("demo has MathML") + "</math>".len();
        let svg = renderer()
            .render_svg(&html[start..end])
            .unwrap_or_else(|_| panic!("demo formula failed"));
        assert!(svg.contains("<path"));
    }
}

/// Converts LaTeX math (without surrounding `$`) to a MathML string.
///
/// With `display` set, the formula is typeset in display style (as in `\[ ... \]`).
#[wasm_bindgen]
pub fn latex_to_mathml(latex: &str, display: bool) -> Result<String, JsError> {
    convert_latex(latex, display).map_err(|e| JsError::new(&e))
}

fn convert_latex(latex: &str, display: bool) -> Result<String, String> {
    convert_latex_raw(latex, display)?
        .map(adapt_for_this_renderer)
        .map_err(|e| format!("LaTeX error: {e}"))
}

/// Where in `latex` the conversion fails, as `[start, end]` in UTF-16 code units (the indices
/// of a JavaScript string, e.g. for `textarea.setSelectionRange`). `undefined` if it converts.
#[wasm_bindgen]
pub fn latex_error_range(latex: &str, display: bool) -> Option<Vec<u32>> {
    let error = convert_latex_raw(latex, display).ok()?.err()?;
    let utf16 = |byte: usize| {
        let byte = byte.min(latex.len());
        // a range may in principle end inside a character; round down to a boundary
        let byte = (0..=byte).rev().find(|&b| latex.is_char_boundary(b)).unwrap_or(0);
        latex[..byte].encode_utf16().count() as u32
    };
    Some(vec![utf16(error.0.start), utf16(error.0.end)])
}

/// The accessible name for a formula.
///
/// The author's own text wins: an `<annotation encoding="text/plain">` inside `<semantics>`, or an
/// `alttext` attribute on `<math>` — MathML defines both for exactly this. Failing that, the text
/// content of the expression is used, which is at least what a screen reader would read out of the
/// markup. This runs on the raw string, before parsing, so a document that fails to parse still gets
/// a name.
pub fn alt_text(mathml: &str) -> String {
    annotation_text(mathml)
        .or_else(|| attribute_value(mathml, "alttext"))
        .unwrap_or_else(|| text_content(mathml))
}

/// The body of the first `<annotation>` whose encoding is `text/plain`.
fn annotation_text(mathml: &str) -> Option<String> {
    let mut rest = mathml;
    while let Some(start) = rest.find("<annotation") {
        let after = &rest[start + "<annotation".len()..];
        let tag_end = after.find('>')?;
        let tag = &after[..tag_end];
        let body = &after[tag_end + 1..];
        let text_end = body.find("</annotation")?;
        rest = &body[text_end..];
        if tag.to_ascii_lowercase().contains("text/plain") {
            let text = collapse(&decode_entities(&body[..text_end]));
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// The value of an attribute on the root `<math>` element.
fn attribute_value(mathml: &str, name: &str) -> Option<String> {
    let start = mathml.find("<math")?;
    let tag = &mathml[start..];
    let tag = &tag[..tag.find('>')?];
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(i) = tag.find(&needle) {
            let rest = &tag[i + needle.len()..];
            if let Some(j) = rest.find(quote) {
                let text = collapse(&decode_entities(&rest[..j]));
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

/// Everything that is text rather than markup, with whitespace collapsed.
fn text_content(mathml: &str) -> String {
    let mut out = String::new();
    let mut rest = mathml;
    loop {
        match rest.find('<') {
            Some(i) => {
                out.push_str(&rest[..i]);
                rest = &rest[i..];
                if rest.starts_with("<!--") {
                    match rest.find("-->") {
                        Some(k) => {
                            rest = &rest[k + 3..];
                            continue;
                        }
                        None => break,
                    }
                }
                match rest.find('>') {
                    Some(k) => rest = &rest[k + 1..],
                    None => break,
                }
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    collapse(&decode_entities(&out))
}

/// Decodes the entities that carry meaning in a name; anything else is left alone.
fn decode_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&amp;", "&")
}

/// One line, single spaces.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

type LatexResult = Result<String, Box<math_core::LatexError>>;

fn convert_latex_raw(latex: &str, display: bool) -> Result<LatexResult, String> {
    use math_core::{LatexToMathML, MathCoreConfig, MathDisplay};

    let converter = LatexToMathML::new(MathCoreConfig::default(), Vec::new())
        .map_err(|e| format!("invalid macro definition: {e:?}"))?;
    let display = if display {
        MathDisplay::Block
    } else {
        MathDisplay::Inline
    };
    Ok(converter
        .convert_with_local_state(latex, display)
        .map(|result| result.mathml))
}

/// math-core targets browsers implementing MathML Core. Two details differ from what this
/// renderer expects:
///
/// - `\overline` is an accent over the base written with U+0332 COMBINING LOW LINE. That glyph
///   sits below the baseline here and would be drawn through the base, so use U+0305
///   COMBINING OVERLINE for accents over a base.
/// - MathML Core tables have no spacing between rows and columns other than the cell padding,
///   whereas the MathML 3 defaults used here add 0.8em and 0.5ex on top of it.
fn adapt_for_this_renderer(mathml: String) -> String {
    mathml
        .replace("\u{332}</mo></mover>", "\u{305}</mo></mover>")
        .replace("<mtable", "<mtable columnspacing=\"0em\" rowspacing=\"0em\"")
}

#[cfg(test)]
mod latex_tests {
    use super::tests::renderer;
    use super::*;

    fn svg(renderer: &MathRenderer, latex: &str) -> String {
        let mathml = convert_latex(latex, true).unwrap_or_else(|e| panic!("{e}"));
        renderer
            .render_svg(&mathml)
            .unwrap_or_else(|_| panic!("render failed for {latex}"))
    }

    fn view_box_width(svg: &str) -> f32 {
        let start = svg.find("viewBox=\"").expect("viewBox") + "viewBox=\"".len();
        let numbers: Vec<f32> = svg[start..]
            .split('"')
            .next()
            .unwrap()
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        numbers[2]
    }

    #[test]
    fn converts_common_formulas() {
        let r = renderer();
        for latex in [
            r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
            r"\sum_{n=1}^\infty \frac1{n^2} = \frac{\pi^2}{6}",
            r"\int_0^\infty e^{-x^2}\,dx",
            r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
            r"\mathbb{R}\ \mathcal{L}\ \hat{x}\ \overline{AB}\ \left(\frac12\right)",
            r"\begin{aligned} a &= b + c \\ &= d \end{aligned}",
        ] {
            assert!(svg(&r, latex).contains("<path"), "{latex}");
        }
    }

    #[test]
    fn reports_latex_errors() {
        assert!(convert_latex(r"\frac{1", true).is_err());
        assert!(convert_latex(r"\notacommand", true).is_err());
    }

    #[test]
    fn error_range_points_into_the_source() {
        assert_eq!(latex_error_range(r"\frac{1}{2}", true), None);
        let src = r"x + \notacommand y";
        let range = latex_error_range(src, true).expect("error expected");
        assert_eq!(&src[range[0] as usize..range[1] as usize], r"\notacommand");
        // indices count UTF-16 code units, like JavaScript strings
        let src = "αβ + \\notacommand";
        let range = latex_error_range(src, true).expect("error expected");
        let units: Vec<u16> = src.encode_utf16().collect();
        let marked = String::from_utf16(&units[range[0] as usize..range[1] as usize]).unwrap();
        assert_eq!(marked, r"\notacommand");
    }

    #[test]
    fn overline_is_an_accent_above_the_base() {
        let mathml = convert_latex(r"\overline{AB}", true).unwrap();
        assert!(mathml.contains("\u{305}</mo></mover>"), "{mathml}");
        assert!(!mathml.contains("\u{332}"), "{mathml}");
        // \underline keeps the low line
        let under = convert_latex(r"\underline{AB}", true).unwrap();
        assert!(under.contains("\u{332}"), "{under}");
    }

    #[test]
    fn aligned_uses_the_cell_padding_from_the_inline_style() {
        // math-core writes `padding-left: 0` etc. into the cells of `aligned`; without it the
        // two columns would be 0.8em + 2 * 0.4em further apart.
        let r = renderer();
        let aligned = svg(&r, r"\begin{aligned} a &= b \end{aligned}");
        let padded = r
            .render_svg(
                "<math><mtable><mtr><mtd><mi>a</mi></mtd><mtd><mo>=</mo><mi>b</mi></mtd></mtr></mtable></math>",
            )
            .unwrap_or_else(|_| panic!());
        assert!(view_box_width(&aligned) < view_box_width(&padded) - 500.0);
    }
}
