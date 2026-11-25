# @manuel-rhdt/formel-wasm

Typeset MathML and LaTeX to SVG in the browser with any OpenType **MATH** font, using the layout
engine of [formel](https://github.com/manuel-rhdt/formel). It implements the OpenType MATH
specification and [MathML Core](https://www.w3.org/TR/mathml-core/): script and fraction placement,
stretchy operators, radicals, tables, accents, italic corrections, math kerning.

MathJax and KaTeX are self-contained JavaScript typesetters: they position glyphs from their own
bundled fonts and leave the rest of the layout to the browser. Here the layout is computed in
WebAssembly from the OpenType MATH font **you** load, under the MathML Core rules, with the aim of
reproducing what LuaTeX or XeTeX produce with that same font. Two consequences: the output is an SVG
whose glyph positions are already fixed, and any MATH font works, so the formulas can match the
surrounding text — which is also why **you supply the font**.

## Install

```sh
npm install @manuel-rhdt/formel-wasm
```

## Use

```js
import init, { MathRenderer } from "@manuel-rhdt/formel-wasm";

await init();

const font = new Uint8Array(await (await fetch("/fonts/latinmodern-math.otf")).arrayBuffer());
const renderer = new MathRenderer(font);

document.querySelector("#formula").innerHTML =
  renderer.render_svg('<math><mfrac><mi>a</mi><mi>b</mi></mfrac></math>');
```

`render()` returns the SVG together with its size and the baseline inside it, both in font units
(`units_per_em()` per em), which is what you need to place a formula inline with text:

```js
const { svg, width, height, baseline } = renderer.render("<math><mi>x</mi></math>", false);
```

LaTeX is converted with [math-core](https://github.com/tmke8/math-core) before it is laid out:

```js
import { latex_to_mathml, latex_error_range } from "@manuel-rhdt/formel-wasm";

const mathml = latex_to_mathml("\\int_0^1 x^2\\,\\mathrm{d}x", true); // display style
const range = latex_error_range("\\frac{", true); // [start, end] in UTF-16 units, or undefined
```

All free OpenType MATH fonts work (New Computer Modern, STIX Two, TeX Gyre, Asana, Fira Math, …);
Cambria Math works too but its license does not allow redistribution.

## Placing it in a page

`@manuel-rhdt/formel-wasm/dom` carries the string helpers a page needs — colour, accessible name,
sizing, inline alignment, `data:` URIs — so you do not have to write them:

```js
import { snippets, styleSvg } from "@manuel-rhdt/formel-wasm/dom";

const rendering = renderer.render(mathml, false);
const svg = styleSvg(rendering.svg, { fg: "currentColor", alt: rendering.alt });

const { svg: inline } = snippets({
  svg,
  alt: rendering.alt,
  display: false,
  width: rendering.width,
  height: rendering.height,
  baseline: rendering.baseline,
  unitsPerEm: renderer.units_per_em(),
});

element.innerHTML = inline; // as big as the surrounding text, sitting on its baseline
```

Also there: `svgInPoints` for a standalone file sized in pt, `svgDataUri`/`svgDataUriEncoded` for
`data:` URIs, `setPngResolution` to tag PNGs from a canvas with their dpi, and `htmlImage` for
pasting into rich-text editors. Type declarations are included.

## Bundlers

The module is a wasm-bindgen `--target web` build. It loads its `.wasm` relative to itself
(`import.meta.url`), which Vite, webpack, Rollup and esbuild rewrite for you. If you serve the
JavaScript and the `.wasm` yourself, make sure the server sends `application/wasm`; otherwise the
glue falls back to a slower fetch-and-instantiate path.

## Without a bundler

Both jsDelivr and unpkg serve the ESM build and the module with the right content type:

```js
import init, { MathRenderer } from "https://cdn.jsdelivr.net/npm/@manuel-rhdt/formel-wasm@0.1/+esm";
```

## Licences

MIT for the engine, see [LICENSE](https://github.com/manuel-rhdt/formel/blob/master/LICENSE). The
WebAssembly binary also contains the MIT and Apache-2.0 licensed crates listed in
[THIRD_PARTY.md](https://github.com/manuel-rhdt/formel/blob/master/formel-wasm/npm/THIRD_PARTY.md).
Fonts are not included and keep their own licences.
