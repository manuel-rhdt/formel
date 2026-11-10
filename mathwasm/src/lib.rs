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

    fn renderer() -> MathRenderer {
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
