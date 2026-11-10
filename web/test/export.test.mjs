// Run with: node --test web/test/export.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULTS, escapeXml, styleSvg, sizeSvg, viewBox, snippets, toBase64, svgDataUri,
  escapeMarkdownAlt, encodeState, decodeState,
} from "../export.js";

const RAW = '<svg viewBox="-10 -10 200 100" xmlns="http://www.w3.org/2000/svg">\n' +
  '<g fill="black" stroke="none"><path d="M0 0"/></g>\n</svg>';

test("escapeXml escapes markup characters", () => {
  assert.equal(escapeXml(`a<b>&"c"`), "a&lt;b&gt;&amp;&quot;c&quot;");
});

test("styleSvg recolors and labels", () => {
  const svg = styleSvg(RAW, { fg: "#ff0000", alt: "x < y" });
  assert.ok(svg.includes('fill="#ff0000"'));
  assert.ok(!svg.includes('"black"'));
  assert.ok(svg.includes('role="img" aria-label="x &lt; y"'));
  assert.ok(svg.includes("<title>x &lt; y</title>"));
  assert.ok(!svg.includes("<rect"), "transparent background adds no rect");
});

test("styleSvg adds a background rectangle covering the viewBox", () => {
  const svg = styleSvg(RAW, { bg: "#ffffff", alt: "" });
  assert.ok(svg.includes('<rect x="-10" y="-10" width="200" height="100" fill="#ffffff"/>'));
  // the background comes before the glyphs so it is painted underneath
  assert.ok(svg.indexOf("<rect") < svg.indexOf("<g "));
  assert.ok(!svg.includes("aria-label"), "no alt text, no label");
});

test("styleSvg keeps the SVG well-formed on odd input", () => {
  assert.equal(styleSvg("not svg", { alt: "a" }), "not svg");
});

test("viewBox parses", () => {
  assert.deepEqual(viewBox(RAW), { x: -10, y: -10, width: 200, height: 100 });
  assert.equal(viewBox("<svg>"), null);
});

test("sizeSvg scales font units to pixels", () => {
  const { svg, width, height } = sizeSvg(RAW, 48, 1000);
  assert.ok(Math.abs(width - 9.6) < 1e-9, String(width));
  assert.ok(Math.abs(height - 4.8) < 1e-9, String(height));
  assert.ok(svg.startsWith('<?xml version="1.0" encoding="UTF-8"?>\n<svg width="9.60" height="4.80" viewBox='));
});

test("base64 handles non-ASCII text", () => {
  assert.equal(toBase64("abc"), "YWJj");
  assert.equal(Buffer.from(toBase64("π≈3,14"), "base64").toString("utf8"), "π≈3,14");
  assert.ok(svgDataUri("<svg/>").startsWith("data:image/svg+xml;base64,"));
});

test("base64 handles large inputs without overflowing the stack", () => {
  const big = "x".repeat(500_000);
  assert.equal(Buffer.from(toBase64(big), "base64").length, 500_000);
});

test("inline snippets sit on the baseline", () => {
  const s = snippets({
    svg: RAW, alt: 'E = mc^2 "quoted"', display: false,
    width: 2000, height: 1000, baseline: 800, unitsPerEm: 1000,
  });
  // 1000 units = 1em; 200 units below the baseline = 0.2em
  assert.ok(s.img.includes("height:1em;vertical-align:-0.2em"), s.img);
  assert.ok(s.img.includes('alt="E = mc^2 &quot;quoted&quot;"'), s.img);
  assert.ok(s.img.startsWith('<img src="data:image/svg+xml;base64,'));
  assert.ok(s.svg.includes('width="2em" height="1em" style="vertical-align:-0.2em"'), s.svg);
});

test("display snippets are centered blocks", () => {
  const s = snippets({
    svg: RAW, alt: "x", display: true, width: 3000, height: 1500, baseline: 1000, unitsPerEm: 1000,
  });
  assert.ok(s.img.includes("display:block;margin:1em auto"), s.img);
  assert.ok(!s.img.includes("vertical-align"), s.img);
});

test("markdown snippets escape the alt text", () => {
  assert.equal(escapeMarkdownAlt("\\left[ x \\right]\n"), "\\\\left\\[ x \\\\right\\]");
  const s = snippets({
    svg: RAW, alt: "[x]", display: false, width: 1, height: 1, baseline: 1, unitsPerEm: 1,
  });
  assert.equal(s.markdownFile, "![\\[x\\]](formula.svg)");
  assert.ok(s.markdownData.startsWith("![\\[x\\]](data:image/svg+xml;base64,"));
});

test("state round-trips through the URL fragment", () => {
  const fonts = ["latinmodern-math.otf", "stixtwo-math.otf"];
  const state = {
    ...DEFAULTS, mode: "mathml", source: "<math>\n<mi>x</mi> & é</math>", font: "stixtwo-math.otf",
    display: false, fg: "#FF0000", bg: "#00ff00", size: 5.5, debug: true,
  };
  const { state: back, hasSource } = decodeState("#" + encodeState(state), fonts);
  assert.equal(hasSource, true);
  assert.deepEqual(back, { ...state, fg: "#ff0000" });
});

test("default state is omitted from the fragment", () => {
  const hash = encodeState({ ...DEFAULTS, source: "x" });
  assert.equal(hash, "m=latex&s=x");
});

test("invalid fragments fall back to defaults", () => {
  const { state, hasSource } = decodeState("#m=nope&f=evil.otf&c=zzz&b=12&z=abc&d=maybe", ["a.otf"]);
  assert.equal(hasSource, false);
  assert.deepEqual(state, { ...DEFAULTS, source: "" });
  assert.equal(decodeState("#z=99").state.size, 8);
  assert.equal(decodeState("#z=0").state.size, 1);
});
