//! WebAssembly bindings for `formel`.
//!
//! ```js
//! import init, { MathRenderer } from "./pkg/formel_wasm.js";
//! await init();
//! const font = new Uint8Array(await (await fetch("font.otf")).arrayBuffer());
//! const renderer = new MathRenderer(font);
//! container.innerHTML = renderer.render_svg("<math><mi>x</mi></math>");
//! ```

use formel::mathmlparser;
use formel::shaper::Shaper;
use formel::speech;
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
        let alt =
            authored_alt_text(mathml).unwrap_or_else(|| speech::speak_document(&document, root));
        let svg = svg_renderer::render_to_string_with(
            &document,
            &typeset.layouts,
            self.shaper.face(),
            units_per_em,
            flags,
            svg_renderer::A11y::titled(&alt),
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
        // Without an author's name, the formula is read from its structure.
        assert_eq!(
            alt_text("<math><mi>x</mi><mo>&#x2212;</mo><mn>1</mn></math>"),
            "x minus 1"
        );
        assert_eq!(
            alt_text("<math><msqrt><mi>x</mi><mo>+</mo><mn>1</mn></msqrt></math>"),
            "the square root of x plus 1"
        );
        // Markup that does not parse still gets its text content, with references decoded
        // ("x−1", not "x&#x2212;1").
        assert_eq!(
            alt_text("<math><mi>x</mi><mo>&#x2212;</mo><mn>1</mn></mrow>"),
            "x\u{2212}1"
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

    /// The guard P0 asked for: whatever the input, a rendered SVG carries a name and declares itself
    /// an image. Runs over every demo equation, so a regression in the fallback shows up in CI
    /// rather than in a screen reader.
    #[test]
    fn every_demo_equation_renders_an_accessible_svg() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/equations");
        let r = renderer();
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("demo equations must be readable: {e}"))
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("xml"))
            .collect();
        paths.sort();
        assert!(!paths.is_empty(), "expected demo equations");
        for path in paths {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let mathml = std::fs::read_to_string(&path).unwrap();
            let out = r
                .render(&mathml, false)
                .unwrap_or_else(|_| panic!("{name}: render failed"));
            assert!(
                !out.alt().trim().is_empty(),
                "{name}: the accessible name is empty"
            );
            let svg = out.svg();
            assert!(svg.contains(r#"role="img""#), "{name}: no role: {svg}");
            let title = svg
                .find("<title>")
                .unwrap_or_else(|| panic!("{name}: no <title>"));
            // The title has to come before the drawing: some readers take the first child only.
            assert_eq!(
                svg[..title].matches('<').count(),
                1,
                "{name}: <title> is not the SVG's first child"
            );
        }
    }

    #[test]
    fn every_demo_font_loads_and_renders() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/fonts");
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if !matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("otf" | "ttf")
            ) {
                continue;
            }
            let name = path.display().to_string();
            let font = std::fs::read(&path).unwrap();
            let renderer =
                MathRenderer::new(&font).unwrap_or_else(|_| panic!("{name}: no MATH table"));
            let svg = convert_latex(r"\sum_{i=0}^n \frac{x_i}{\sqrt{2}} \left(a+b\right)", true)
                .and_then(|m| {
                    renderer
                        .render_svg(&m)
                        .map_err(|_| "render failed".to_string())
                })
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
        assert!(
            svg.contains(&format!("{} {}", w, h)),
            "viewBox should match: {svg}"
        );
        // the baseline is inside the box, and a fraction has stuff below it
        assert!(result.baseline() > 0.0 && result.baseline() < h);
        // a lone `x` sits on the baseline, so there is hardly anything below it
        let x = r
            .render("<math><mi>x</mi></math>", false)
            .unwrap_or_else(|_| panic!());
        assert!(x.height() - x.baseline() < 0.3 * r.units_per_em() as f32);
    }

    #[test]
    fn math_is_inline_unless_display_block() {
        let r = renderer();
        let size = |mathml: &str| {
            let out = r
                .render(mathml, false)
                .unwrap_or_else(|_| panic!("{mathml}"));
            (out.width(), out.height())
        };
        let sum = "<munderover><mo>∑</mo><mi>a</mi><mi>b</mi></munderover>";
        let inline = size(&format!("<math>{sum}</math>"));
        let block = size(&format!("<math display=\"block\">{sum}</math>"));
        assert_eq!(
            inline,
            size(&format!("<math display=\"inline\">{sum}</math>"))
        );
        // inline: small operator with limits at the side; display: large with limits stacked
        assert!(
            inline.1 < block.1 && inline.0 > block.0,
            "{inline:?} {block:?}"
        );
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
        .map(|mathml| with_tex_annotation(mathml, latex))
        .map_err(|e| format!("LaTeX error: {e}"))
}

/// Keeps the LaTeX source in the MathML, the standard way: the content goes into
/// `<semantics>` with an `<annotation encoding="application/x-tex">`.
///
/// `<math display="block"><mi>x</mi></math>` becomes
/// `<math display="block"><semantics><mrow><mi>x</mi></mrow><annotation
/// encoding="application/x-tex">x</annotation></semantics></math>`.
///
/// Renderers (this one included) lay out only the first child of `<semantics>`, and the
/// accessible name ignores this annotation, which is not `text/plain`, so neither changes. An
/// empty formula is left alone.
fn with_tex_annotation(mathml: String, latex: &str) -> String {
    let Some(open_end) = mathml.find('>').map(|i| i + 1) else {
        return mathml;
    };
    let Some(close_start) = mathml.rfind("</math>") else {
        return mathml;
    };
    if !mathml.starts_with("<math") || close_start < open_end {
        return mathml;
    }
    let content = &mathml[open_end..close_start];
    if content.trim().is_empty() {
        return mathml;
    }
    let tex = latex
        .trim()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "{}<semantics><mrow>{content}</mrow><annotation encoding=\"application/x-tex\">{tex}</annotation></semantics></math>",
        &mathml[..open_end]
    )
}

/// Where in `latex` the conversion fails, as `[start, end]` in UTF-16 code units (the indices
/// of a JavaScript string, e.g. for `textarea.setSelectionRange`). `undefined` if it converts.
#[wasm_bindgen]
pub fn latex_error_range(latex: &str, display: bool) -> Option<Vec<u32>> {
    let error = convert_latex_raw(latex, display).ok()?.err()?;
    let utf16 = |byte: usize| {
        let byte = byte.min(latex.len());
        // a range may in principle end inside a character; round down to a boundary
        let byte = (0..=byte)
            .rev()
            .find(|&b| latex.is_char_boundary(b))
            .unwrap_or(0);
        latex[..byte].encode_utf16().count() as u32
    };
    Some(vec![utf16(error.0.start), utf16(error.0.end)])
}

/// The accessible name for a formula.
///
/// The author's own text wins: an `<annotation encoding="text/plain">` inside `<semantics>`, or an
/// `alttext` attribute on `<math>` — MathML defines both for exactly this. Failing that, the
/// formula is read aloud from its structure ([`formel::speech`]: "x plus 1", "the square root of
/// x"). Only markup that does not parse falls back to its bare text content, so even that gets a
/// name.
pub fn alt_text(mathml: &str) -> String {
    authored_alt_text(mathml)
        .or_else(|| {
            let document = mathmlparser::parse(mathml.as_bytes()).ok()?;
            let root = document.root()?;
            // Nothing to read: an empty name, not "blank".
            if document.node(root).children.is_empty() {
                return None;
            }
            Some(speech::speak_document(&document, root))
        })
        .unwrap_or_else(|| text_content(mathml))
}

/// The author's accessible name: a `text/plain` annotation or the `alttext` of `<math>`.
fn authored_alt_text(mathml: &str) -> Option<String> {
    annotation_text(mathml).or_else(|| attribute_value(mathml, "alttext"))
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

/// Decodes the character references that carry meaning in a name. Unknown named entities are left
/// as written rather than dropped, so a name that says `&minus;` is visibly wrong instead of silently
/// shorter than the markup.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        // A reference is short; `;` further away than this is ordinary text.
        let decoded = tail
            .find(';')
            .filter(|end| *end <= 10)
            .and_then(|end| decode_reference(&tail[1..end]).map(|c| (c, end + 1)));
        match decoded {
            Some((character, len)) => {
                out.push(character);
                rest = &tail[len..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// One character reference body: `#8722`, `#x2212` or a name.
fn decode_reference(body: &str) -> Option<char> {
    if let Some(number) = body.strip_prefix('#') {
        let code = match number
            .strip_prefix('x')
            .or_else(|| number.strip_prefix('X'))
        {
            Some(hexadecimal) => u32::from_str_radix(hexadecimal, 16).ok()?,
            None => number.parse::<u32>().ok()?,
        };
        return char::from_u32(code);
    }
    Some(match body {
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "amp" => '&',
        _ => return None,
    })
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

/// math-core targets browsers implementing MathML Core. Three details differ from what this
/// renderer expects:
///
/// - `\overline` is an accent over the base written with U+0332 COMBINING LOW LINE. That glyph
///   sits below the baseline here and would be drawn through the base, so use U+0305
///   COMBINING OVERLINE for accents over a base.
/// - MathML Core tables have no spacing between rows and columns other than the cell padding,
///   whereas the MathML 3 defaults used here add 0.8em and 0.5ex on top of it.
/// - After `\sin`, `\lim` and the like it writes the thin space as `lspace="0.1667em"` on the
///   invisible function application operator U+2061, which browsers need. This renderer already
///   puts a thin space after that operator (except before an opening fence, as TeX does), so the
///   attribute would add the space twice.
/// - TeX adds the italic correction of a letter before the next letter (`b` and `c` in `abc`).
///   MathML Core adds it before the next child of a row only if that child is not slanted, so
///   between two slanted letters there is none. [`separate_letters`] puts an empty `<mspace/>`
///   between them: it is not slanted, so it receives the correction, and it takes no room itself.
fn adapt_for_this_renderer(mathml: String) -> String {
    separate_letters(&mathml)
        .replace(
            "<mo lspace=\"0.1667em\">&ApplyFunction;</mo>",
            "<mo>&ApplyFunction;</mo>",
        )
        .replace("\u{332}</mo></mover>", "\u{305}</mo></mover>")
        .replace(
            "<mtable",
            "<mtable columnspacing=\"0em\" rowspacing=\"0em\"",
        )
}

/// Puts `<mspace/>` between two neighbouring single-character `<mi>`s of a row, as in `abc`, so
/// that the first one's italic correction is applied (see [`adapt_for_this_renderer`]).
///
/// Only children of elements that lay out their children as a row are separated: in
/// `<msub><mi>x</mi><mi>i</mi></msub>` or `<munderover>` the two letters are the arguments of the
/// element, and a third child would break it. Only the bare `<mi>x</mi>` form is matched: letters
/// with attributes (`mathvariant="normal"`) are upright, and so are words like `<mi>sin</mi>`.
/// Whether a letter has a correction is up to the font; where it has none, the marker changes
/// nothing.
fn separate_letters(mathml: &str) -> String {
    /// Elements that lay all their children out in a row (`<msqrt>` and the rest by inferred
    /// `<mrow>`).
    const ROWS: [&str; 9] = [
        "math", "mrow", "mstyle", "msqrt", "mtd", "mpadded", "mphantom", "menclose", "merror",
    ];
    /// The length of the `<mi>x</mi>` at the start of `markup`, for one character `x` that is
    /// not markup or an entity.
    fn bare_letter(markup: &str) -> Option<usize> {
        let letter = markup.strip_prefix("<mi>")?.chars().next()?;
        let end = "<mi>".len() + letter.len_utf8();
        (!matches!(letter, '<' | '>' | '&') && markup[end..].starts_with("</mi>"))
            .then_some(end + "</mi>".len())
    }

    let mut out = String::with_capacity(mathml.len() + 32);
    // the open elements, each with whether its latest child is a bare letter
    let mut open: Vec<(&str, bool)> = vec![("", false)];
    let mut rest = mathml;
    while !rest.is_empty() {
        let Some(tag_start) = rest.find('<') else {
            out.push_str(rest);
            break;
        };
        if tag_start > 0 {
            // text between tags: not a letter element
            out.push_str(&rest[..tag_start]);
            open.last_mut().expect("root").1 = false;
            rest = &rest[tag_start..];
        }
        if let Some(len) = bare_letter(rest) {
            let (parent, after_letter) = open.last_mut().expect("root");
            if *after_letter && ROWS.contains(parent) {
                out.push_str("<mspace/>");
            }
            *after_letter = true;
            out.push_str(&rest[..len]);
            rest = &rest[len..];
            continue;
        }
        let Some(tag_end) = rest.find('>') else {
            out.push_str(rest);
            break;
        };
        let tag = &rest[..=tag_end];
        if tag.starts_with("</") {
            if open.len() > 1 {
                open.pop();
            }
            open.last_mut().expect("root").1 = false;
        } else {
            open.last_mut().expect("root").1 = false;
            if !tag.ends_with("/>") && !tag.starts_with("<!") && !tag.starts_with("<?") {
                let name = tag[1..].split(|c: char| c.is_whitespace() || c == '>').next();
                open.push((name.unwrap_or(""), false));
            }
        }
        out.push_str(tag);
        rest = &rest[tag_end + 1..];
    }
    out
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
    fn neighbouring_letters_are_separated_for_their_italic_correction() {
        assert_eq!(
            separate_letters("<math><mi>a</mi><mi>b</mi><mi>c</mi></math>"),
            "<math><mi>a</mi><mspace/><mi>b</mi><mspace/><mi>c</mi></math>"
        );
        // Greek and other multi-byte letters count as one, and so do rows of other elements
        assert_eq!(
            separate_letters("<msqrt><mi>f</mi><mi>α</mi></msqrt>"),
            "<msqrt><mi>f</mi><mspace/><mi>α</mi></msqrt>"
        );
        assert_eq!(
            separate_letters("<mrow><mi>a</mi><mi>b</mi><mo>+</mo><mi>c</mi><mi>d</mi></mrow>"),
            "<mrow><mi>a</mi><mspace/><mi>b</mi><mo>+</mo><mi>c</mi><mspace/><mi>d</mi></mrow>"
        );
        // not where the letters are arguments of an element rather than a row
        for unchanged in [
            "<msub><mi>x</mi><mi>i</mi></msub>",
            "<msubsup><mi>x</mi><mi>i</mi><mi>j</mi></msubsup>",
            "<mfrac><mi>a</mi><mi>b</mi></mfrac>",
            "<munderover><mo>∑</mo><mi>a</mi><mi>b</mi></munderover>",
            "<mroot><mi>x</mi><mi>n</mi></mroot>",
            // not between a letter and anything else, or around words and upright letters
            "<mrow><mi>f</mi><mn>2</mn></mrow>",
            "<mrow><mi>f</mi><mo>+</mo><mi>x</mi></mrow>",
            "<mrow><mi>f</mi><mrow><mi>x</mi></mrow></mrow>",
            "<mrow><mi>sin</mi><mi>x</mi></mrow>",
            "<mrow><mi>x</mi><mi>sin</mi></mrow>",
            "<mrow><mi mathvariant=\"normal\">n</mi><mi>x</mi></mrow>",
            "<mrow><mi>x</mi><mi mathvariant=\"normal\">n</mi></mrow>",
            "<mrow><mi>&amp;</mi><mi>x</mi></mrow>",
        ] {
            assert_eq!(separate_letters(unchanged), unchanged);
        }
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

#[cfg(test)]
mod latex_alt_tests {
    use super::*;

    /// LaTeX → math-core → speech, end to end. A change in math-core's output that breaks how a
    /// construct is recognised shows up here.
    #[test]
    fn latex_speech() {
        let cases = [
            (
                r"\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}",
                "the sum from n equals 1 to infinity of 1 over n squared equals pi squared over 6",
            ),
            (
                r"\int_a^b f(x)\,dx = F(b) - F(a)",
                "the integral from a to b of f of x d x equals capital F of b minus capital F of a",
            ),
            (
                r"\lim_{x \to 0} \frac{\sin x}{x} = 1",
                "the limit as x approaches 0 of the fraction with numerator sine of x and denominator x equals 1",
            ),
            (
                r"\frac{d}{dx} f(x)",
                "the derivative with respect to x of f of x",
            ),
            (
                r"\frac{dy}{dx} = 2x",
                "the derivative of y with respect to x equals 2 x",
            ),
            (
                r"\frac{d^2 y}{dx^2}",
                "the second derivative of y with respect to x",
            ),
            (
                r"\frac{\partial^2 f}{\partial x \partial y}",
                "the second partial derivative of f with respect to x and y",
            ),
            (
                r"i\hbar \frac{\partial}{\partial t}\Psi = \hat H \Psi",
                "i h bar the partial derivative with respect to t of capital psi equals capital H hat capital psi",
            ),
            (r"\binom{n}{k}", "n choose k"),
            (
                r"E = \frac{1}{2} m v^2",
                "capital E equals one half m v squared",
            ),
            (
                r"\frac{1}{2} + \frac{3}{4} = \frac{5}{4}",
                "one half plus three quarters equals five quarters",
            ),
            (r"\tfrac{11}{12}", "11 over 12"),
            (
                r"|x| = \begin{cases} x & x \ge 0 \\ -x & x < 0 \end{cases}",
                "the absolute value of x equals 2 cases; case 1: x, if x is greater than or equal to 0; \
              case 2: negative x, if x is less than 0; end cases",
            ),
            (
                r"\begin{cases} 1 & \text{if } x > 0 \\ 0 & \text{otherwise} \end{cases}",
                "2 cases; case 1: 1, if x is greater than 0; case 2: 0, otherwise; end cases",
            ),
            (
                r"\{ x \in \mathbb{R} : x > 0 \}",
                "the set of all x in double-struck capital R such that x is greater than 0",
            ),
            (r"\{1, 2, 3\}", "the set 1 comma 2 comma 3"),
            (
                r"P(A \mid B)",
                "capital P open paren capital A given capital B close paren",
            ),
            (r"f(x) = \sin(2x)", "f of x equals sine of 2 x"),
            (r"g(f(x))", "g of open paren f of x close paren"),
            (r"a(b + c)", "a open paren b plus c close paren"),
            (
                r"a \equiv b \pmod{n}",
                "a is equivalent to b open paren mod n close paren",
            ),
            (
                r"f^{-1}(x) + A^T",
                "f inverse of x plus capital A transpose",
            ),
            (r"90^\circ", "90 degrees"),
            (
                r"x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}",
                "x equals the fraction with numerator negative b plus or minus the square root of b \
              squared minus 4 a c and denominator 2 a",
            ),
            (
                r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
                "the 2 by 2 matrix; row 1: a, b; row 2: c, d; end matrix",
            ),
        ];
        let mut failures = Vec::new();
        for (latex, expected) in cases {
            let spoken = alt_text(&convert_latex(latex, true).unwrap());
            if spoken != expected {
                failures.push(format!(
                    "{latex}\n  got:      {spoken}\n  expected: {expected}"
                ));
            }
        }
        assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    }

    #[test]
    fn latex_source_is_kept_as_an_annotation() {
        assert_eq!(
            convert_latex("x", true).unwrap(),
            "<math display=\"block\"><semantics><mrow><mi>x</mi></mrow>\
             <annotation encoding=\"application/x-tex\">x</annotation></semantics></math>"
        );
        // escaped for XML, and parsed back to the original source
        let mathml = convert_latex(r" a < b \& c > d ", false).unwrap();
        assert!(
            mathml.ends_with(
                r#"<annotation encoding="application/x-tex">a &lt; b \&amp; c &gt; d</annotation></semantics></math>"#
            ),
            "{mathml}"
        );
        let document = mathmlparser::parse(mathml.as_bytes()).unwrap();
        assert!(document.root().is_some());
        // nothing to annotate
        assert_eq!(convert_latex("", false).unwrap(), "<math></math>");
    }

    /// The annotation changes neither the drawing nor the accessible name.
    #[test]
    fn annotation_changes_nothing_visible() {
        let r = tests::renderer();
        for latex in [
            r"\frac{a}{b}",
            r"\sum_{i=1}^n x_i",
            r"\begin{pmatrix}a&b\\c&d\end{pmatrix}",
        ] {
            let plain = convert_latex_raw(latex, true)
                .unwrap()
                .map(adapt_for_this_renderer)
                .unwrap();
            let annotated = convert_latex(latex, true).unwrap();
            let (a, b) = (r.render(&plain, false), r.render(&annotated, false));
            let (a, b) = (
                a.unwrap_or_else(|_| panic!()),
                b.unwrap_or_else(|_| panic!()),
            );
            assert_eq!(a.svg(), b.svg(), "{latex}");
            assert_eq!(a.alt(), b.alt(), "{latex}");
        }
    }

    /// LaTeX input is named by speech, not by its source.
    #[test]
    fn latex_formula_is_named_by_speech() {
        let mathml = convert_latex(r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", true).unwrap();
        let out = tests::renderer()
            .render(&mathml, false)
            .unwrap_or_else(|_| panic!("render failed"));
        assert_eq!(
            out.alt(),
            "the fraction with numerator negative b plus or minus the square root of b squared \
             minus 4 a c and denominator 2 a",
            "{mathml}"
        );
    }
}
