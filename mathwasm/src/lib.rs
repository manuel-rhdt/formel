//! WebAssembly bindings for `math-render`.
//!
//! ```js
//! import init, { MathRenderer } from "./pkg/mathwasm.js";
//! await init();
//! const font = new Uint8Array(await (await fetch("font.otf")).arrayBuffer());
//! const renderer = new MathRenderer(font);
//! container.innerHTML = renderer.render_svg("<math><mi>x</mi></math>");
//! ```

use math_render::mathmlparser;
use math_render::shaper::Shaper;
use math_render::standalone::StyleOptions;
use math_render::svg_renderer;
use wasm_bindgen::prelude::*;

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
        use math_render::host::MathFont;
        self.shaper.units_per_em()
    }

    /// Renders a MathML string (a `<math>` element or a fragment with one root) to an SVG
    /// string. The SVG is sized in font design units, so scale it with CSS (`height: 2em`).
    pub fn render_svg(&self, mathml: &str) -> Result<String, JsError> {
        self.render(mathml, false)
    }

    /// Like [`render_svg`](Self::render_svg) but also outlines the box of every element.
    pub fn render_svg_debug(&self, mathml: &str) -> Result<String, JsError> {
        self.render(mathml, true)
    }
}

impl MathRenderer {
    fn render(&self, mathml: &str, debug: bool) -> Result<String, JsError> {
        use math_render::host::MathFont;

        let document = mathmlparser::parse(mathml.as_bytes())
            .map_err(|e| JsError::new(&format!("could not parse MathML: {e}")))?;
        if document.root().is_none() {
            return Err(JsError::new("the MathML document is empty"));
        }

        // A font size of one em per font unit makes px equal font units.
        let units_per_em = self.shaper.units_per_em() as f32;
        let options = StyleOptions {
            font_size: units_per_em,
            ..Default::default()
        };
        let typeset = math_render::layout(&document, &self.shaper, options);
        let flags = svg_renderer::Flags {
            show_logical_bounds: debug,
            ..Default::default()
        };
        Ok(svg_renderer::render_to_string(
            &document,
            &typeset.layouts,
            self.shaper.face(),
            units_per_em,
            flags,
        ))
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
                .and_then(|m| renderer.render(&m, false).map_err(|_| "render failed".into()))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(svg.contains("<path"), "{name}");
            count += 1;
        }
        assert!(count >= 5, "expected the demo fonts, found {count}");
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
    use math_core::{LatexToMathML, MathCoreConfig, MathDisplay};

    let converter = LatexToMathML::new(MathCoreConfig::default(), Vec::new())
        .map_err(|e| format!("invalid macro definition: {e:?}"))?;
    let display = if display {
        MathDisplay::Block
    } else {
        MathDisplay::Inline
    };
    converter
        .convert_with_local_state(latex, display)
        .map(|result| adapt_for_this_renderer(result.mathml))
        .map_err(|e| format!("LaTeX error: {e}"))
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
