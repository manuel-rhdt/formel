// The package's `./dom` subpath: the helpers `web/dom.js` ships as `@manuel-rhdt/formel-wasm/dom`.
//
// Run with: node --test web/test/dom.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import { snippets, styleSvg, sizeSvg, svgDataUriEncoded, htmlImage } from "../dom.js";

// What the renderer writes now: a viewBox in font units and a size in em.
const RENDERED =
  '<svg width="3.200em" height="1.100em" role="img" viewBox="-50 -50 3200 1100">' +
  '<title>a over b</title><g fill="black" stroke="none"><path d="M0 0"/></g></svg>';

test("snippets does not add a second width and height", () => {
  const { svg } = snippets({
    svg: RENDERED,
    alt: "a over b",
    display: false,
    width: 3200,
    height: 1100,
    baseline: 900,
    unitsPerEm: 1000,
  });
  const open = svg.match(/^<svg[^>]*>/)[0];
  assert.equal((open.match(/width=/g) || []).length, 1, open);
  assert.equal((open.match(/height=/g) || []).length, 1, open);
  assert.match(open, /height="1\.1em" style="vertical-align:-0\.2em"/);
});

test("snippets aligns a display formula by centring it", () => {
  const { svg } = snippets({
    svg: RENDERED,
    alt: "",
    display: true,
    width: 3200,
    height: 1100,
    baseline: 900,
    unitsPerEm: 1000,
  });
  assert.match(svg, /style="display:block;margin:1em auto"/);
});

test("sizeSvg replaces the em size the renderer wrote", () => {
  const { svg, width, height } = sizeSvg(RENDERED, 48, 1000);
  assert.ok(Math.abs(width - 153.6) < 1e-9, String(width));
  assert.ok(Math.abs(height - 52.8) < 1e-9, String(height));
  assert.equal((svg.match(/width=/g) || []).length, 1);
});

test("styleSvg keeps the title the renderer wrote", () => {
  const styled = styleSvg(RENDERED, { fg: "#336699", alt: "a over b" });
  assert.equal((styled.match(/<title/g) || []).length, 1);
  assert.match(styled, /fill="#336699"/);
});

test("htmlImage escapes the alt text", () => {
  const img = htmlImage({ src: "x.svg", alt: 'a < b "c"', width: 10.4, height: 5 });
  assert.match(img, /alt="a &lt; b &quot;c&quot;"/);
  assert.match(img, /width="10" height="5"/);
});

test("svgDataUriEncoded escapes what breaks an attribute, not everything", () => {
  const uri = svgDataUriEncoded('<svg viewBox="0 0 1 1"><path d="M0 0"/></svg>');
  assert.ok(
    uri.startsWith('data:image/svg+xml,%3Csvg viewBox=%220 0 1 1%22%3E%3Cpath d=%22M0 0%22'),
    uri,
  );
  // letters, digits and the path data stay readable, unlike base64
  assert.ok(uri.includes("M0"), uri);
});
