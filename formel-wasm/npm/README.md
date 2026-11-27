# @manuel-rhdt/formel-wasm

Typeset MathML and LaTeX to SVG in the browser with any OpenType **MATH** font, using the layout
engine of [formel](https://github.com/manuel-rhdt/formel). It implements the OpenType MATH
specification and [MathML Core](https://www.w3.org/TR/mathml-core/): script and fraction placement,
stretchy operators, radicals, tables, accents, italic corrections, and math kerning.

MathJax and KaTeX are self-contained JavaScript typesetters: they position glyphs from their own
bundled fonts and leave the rest of the layout to the browser. Here the layout is computed in
WebAssembly from the OpenType MATH font **you** load, under the MathML Core rules, aiming to match
what LuaTeX or XeTeX produce with that same font. The output is an SVG with fixed glyph positions,
and any OpenType MATH font works so formulas can match the surrounding text. Because of that, **you
supply the font**.

## Install

```sh
npm install @manuel-rhdt/formel-wasm
```

## Quick start

```js
import init, { MathRenderer } from "@manuel-rhdt/formel-wasm";
import { inlineHtml } from "@manuel-rhdt/formel-wasm/dom";

await init();
const font = await fetch("/fonts/latinmodern-math.otf").then((r) => r.arrayBuffer());
const renderer = new MathRenderer(new Uint8Array(font));

// Inline math (scaled to 1em, aligned to the text baseline, colored with currentColor):
const eq = renderer.render_latex(String.raw`\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}`, false);
document.querySelector("#inline").innerHTML = inlineHtml(eq, { size: "1em" });

// Display math (centered block):
const block = renderer.render_latex(String.raw`\int_{-\infty}^{\infty} e^{-x^2}\,\mathrm{d}x = \sqrt{\pi}`, true);
document.querySelector("#block").innerHTML = inlineHtml(block, { display: true });
```

All free OpenType MATH fonts work (New Computer Modern, Latin Modern, STIX Two, TeX Gyre, Asana,
Fira Math, Noto Sans Math, …). Cambria Math works too, though its license does not allow web
redistribution.

## API

### `@manuel-rhdt/formel-wasm`

- **`await init()`**: initializes the WebAssembly module.
- **`new MathRenderer(fontBytes)`**: loads an OpenType MATH font from a `Uint8Array`.
  - **`renderer.render_latex(latex, display)`**: converts LaTeX (without `$` delimiters) and renders it, returning a `Rendering`.
  - **`renderer.render(mathml, debug)`**: renders a MathML string, returning a `Rendering`.
  - **`renderer.render_svg(mathml)`**: renders MathML directly to an `<svg>` string.
  - **`renderer.set_font_features("'ss03'")`**: sets CSS `font-feature-settings` (such as stylistic sets) for subsequent renders.
  - **`renderer.add_font_face(fontBytes, unicodeRanges?, sizeAdjust?, fontFeatures?)`**: uses another font for some characters, like a CSS `@font-face` with a `unicode-range`. `unicodeRanges` is a `Uint32Array` of pairs (the first and last codepoint of each range, e.g. `[0x1D49C, 0x1D503]` for the script letters), `sizeAdjust` a factor (`0.9` is 90%) and `fontFeatures` a `font-feature-settings` value. The font is used for a character in its ranges that it has a glyph for; otherwise the faces added before are tried, and last the font of the constructor, which gives the layout constants. Later calls take precedence over earlier ones. The font needs a `MATH` table. See [Mixed fonts](https://github.com/manuel-rhdt/formel#mixed-fonts).
  - **`renderer.units_per_em()`**: returns the font's design units per em.
- **`Rendering`**: `{ svg, alt, width, height, baseline, units_per_em }`, with lengths in font design units.
- **`latex_to_mathml(latex, display)`**: converts LaTeX to MathML via [math-core](https://github.com/tmke8/math-core) without rendering.
- **`latex_error_range(latex, display)`**: returns `[start, end]` in UTF-16 code units when `latex` has a syntax error, or `undefined` if valid.

### `@manuel-rhdt/formel-wasm/dom`

- **`inlineHtml(rendering, { size, display, color, alt })`**: formats a `Rendering` as an `<svg>` element ready for HTML. `size` sets 1em of the formula (`"1em"`, `"1.2em"`, `"18px"`, or a number in px); `display: true` centers it as a block instead of aligning to the text baseline; `color` defaults to `"currentColor"`.
- **`styleSvg(svg, { fg, bg, alt, mathml })`**: sets foreground/background colors, accessible title, and optional embedded MathML `<metadata>`.
- **`sizeSvg(svg, pxPerEm, unitsPerEm)`** / **`svgInPoints(svg, textSizePt, unitsPerEm)`**: produces standalone SVGs with explicit `px` or `pt` dimensions.
- **`svgDataUri(svg)`** / **`svgDataUriEncoded(svg)`**: encodes an SVG as a base64 or percent-encoded `data:image/svg+xml` URI.
- **`setPngResolution(pngBytes, dpi)`** / **`pixelsPerEm(textSizePt, dpi)`**: writes a PNG `pHYs` DPI chunk for hi-DPI raster exports.

## Bundlers and CDN

The package is built with `wasm-bindgen --target web` and resolves its `.wasm` binary via
`import.meta.url`, which Vite, webpack, Rollup, and esbuild handle out of the box. If you host the
files yourself, serve the `.wasm` file with `Content-Type: application/wasm`.

Without a bundler, load the ESM bundle from jsDelivr or unpkg:

```js
import init, { MathRenderer } from "https://cdn.jsdelivr.net/npm/@manuel-rhdt/formel-wasm@0.1/+esm";
```

## License

MIT for the engine ([LICENSE](https://github.com/manuel-rhdt/formel/blob/master/LICENSE)). Third-party
Rust crates compiled into the WebAssembly binary are listed in
[THIRD_PARTY.md](https://github.com/manuel-rhdt/formel/blob/master/formel-wasm/npm/THIRD_PARTY.md).
Fonts are not included and keep their own licenses.
