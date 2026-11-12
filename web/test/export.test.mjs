// Run with: node --test web/test/export.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULTS, escapeXml, styleSvg, mathmlForXml, sizeSvg, viewBox, snippets, toBase64, svgDataUri,
  svgDataUriEncoded, encodeState, decodeState, htmlImage, svgInPoints, pixelsPerEm, setPngResolution,
} from "../export.js";
import { deflateSync, crc32 as zlibCrc32 } from "node:zlib";

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
  assert.ok(/<title id="[^"]+">x &lt; y<\/title>/.test(svg));
  assert.ok(!svg.includes("<rect"), "transparent background adds no rect");
});

test("styleSvg keeps the title the renderer already wrote", () => {
  const titled = '<svg viewBox="-10 -10 200 100" role="img">\n' +
    "<title>x over two</title>\n" +
    '<g fill="black" stroke="none"><path d="M0 0"/></g>\n</svg>';
  const svg = styleSvg(titled, { alt: "ignored" });
  assert.equal(svg.match(/<title/g).length, 1, "exactly one title");
  assert.ok(!svg.includes("ignored"), "the caller's text does not override the renderer's");
  assert.ok(!svg.includes("aria-label"), "no second accessible name");
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
  assert.ok(s.img.startsWith('<img src="data:image/svg+xml,%3Csvg '), s.img);
  assert.ok(s.svg.includes('width="2em" height="1em" style="vertical-align:-0.2em"'), s.svg);
});

test("display snippets are centered blocks", () => {
  const s = snippets({
    svg: RAW, alt: "x", display: true, width: 3000, height: 1500, baseline: 1000, unitsPerEm: 1000,
  });
  assert.ok(s.img.includes("display:block;margin:1em auto"), s.img);
  assert.ok(!s.img.includes("vertical-align"), s.img);
  assert.ok(s.svg.includes("display:block;margin:1em auto"), s.svg);
});

test("the img snippet can use another SVG than the inline one", () => {
  const inline = RAW.replace('"black"', '"currentColor"');
  const s = snippets({
    svg: inline, imgSvg: RAW, mathml: "<math/>", alt: "x", display: false,
    width: 1000, height: 1000, baseline: 800, unitsPerEm: 1000,
  });
  assert.ok(s.svg.includes('fill="currentColor"'));
  assert.ok(!s.img.includes("currentColor"));
  assert.ok(s.img.includes("fill=%22black%22"));
  assert.equal(s.mathml, "<math/>");
});

test("svgDataUriEncoded survives an HTML attribute and a round trip", () => {
  const svg = '<?xml version="1.0"?>\n<svg a="1 & 2" b="#fff">\n  <path d="M0 0"/>100%</svg>';
  const uri = svgDataUriEncoded(svg);
  assert.ok(uri.startsWith("data:image/svg+xml,%3Csvg "), uri);
  for (const bad of ['"', "<", ">", "#", "&", "\n"]) assert.ok(!uri.slice(uri.indexOf(",")).includes(bad), `${JSON.stringify(bad)} in ${uri}`);
  assert.equal(decodeURIComponent(uri.slice(uri.indexOf(",") + 1)), '<svg a="1 & 2" b="#fff"> <path d="M0 0"/>100%</svg>');
  assert.ok(uri.length < svgDataUri(svg).length, "shorter than base64");
});

test("state round-trips through the URL fragment", () => {
  const fonts = ["newcomputermodern-math.otf", "stixtwo-math.otf"];
  const state = {
    ...DEFAULTS, mode: "mathml", source: "<math>\n<mi>x</mi> & é</math>", font: "stixtwo-math.otf",
    display: false, fg: "#FF0000", bg: "#00ff00", size: 5.5, textSize: 10.5,
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

test("htmlImage sets the pixel size and escapes the alt text", () => {
  assert.equal(
    htmlImage({ src: "data:image/png;base64,AA==", alt: 'a "b" <c>', width: 100.4, height: 20.6 }),
    '<img src="data:image/png;base64,AA==" alt="a &quot;b&quot; &lt;c&gt;" width="100" height="21">',
  );
  assert.equal(htmlImage({ src: "x" }), '<img src="x" alt="">');
});

test("svgInPoints sizes the SVG for the text size", () => {
  const { svg, widthPt, heightPt } = svgInPoints(RAW, 12, 1000);
  // viewBox 200 x 100 units, 1000 units per em, 12 pt per em
  assert.ok(Math.abs(widthPt - 2.4) < 1e-9 && Math.abs(heightPt - 1.2) < 1e-9);
  assert.ok(svg.includes('<svg width="2.400pt" height="1.200pt" viewBox='), svg);
});

test("pixelsPerEm converts text size and dpi", () => {
  assert.equal(pixelsPerEm(12, 300), 50);
  assert.equal(pixelsPerEm(72, 96), 96);
});

/** A minimal valid 1x1 PNG, optionally with a pHYs chunk already in it. */
function tinyPng(withPhys = false) {
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4); crc.writeUInt32BE(zlibCrc32(body));
    return Buffer.concat([len, body, crc]);
  };
  const ihdr = Buffer.from([0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]);
  const phys = Buffer.from([0, 0, 0x0b, 0x13, 0, 0, 0x0b, 0x13, 1]);
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr),
    ...(withPhys ? [chunk("pHYs", phys)] : []),
    chunk("IDAT", deflateSync(Buffer.from([0, 0, 0, 0, 0]))), chunk("IEND", Buffer.alloc(0)),
  ]);
}

function chunks(png) {
  const b = Buffer.from(png); const out = []; let at = 8;
  while (at < b.length) {
    const len = b.readUInt32BE(at); const type = b.toString("ascii", at + 4, at + 8);
    const data = b.subarray(at + 8, at + 8 + len);
    const crcOk = b.readUInt32BE(at + 8 + len) === zlibCrc32(b.subarray(at + 4, at + 8 + len));
    out.push({ type, data, crcOk }); at += 12 + len;
  }
  return out;
}

test("setPngResolution inserts a valid pHYs chunk after IHDR", () => {
  const out = chunks(setPngResolution(tinyPng(), 300));
  assert.deepEqual(out.map((c) => c.type), ["IHDR", "pHYs", "IDAT", "IEND"]);
  assert.ok(out.every((c) => c.crcOk));
  const phys = out[1].data;
  assert.equal(phys.readUInt32BE(0), 11811);   // 300 dpi = 11811 px/m
  assert.equal(phys.readUInt32BE(4), 11811);
  assert.equal(phys[8], 1);
});

test("setPngResolution replaces an existing pHYs and ignores non-PNG input", () => {
  const out = chunks(setPngResolution(tinyPng(true), 600));
  assert.deepEqual(out.map((c) => c.type), ["IHDR", "pHYs", "IDAT", "IEND"]);
  assert.equal(out[1].data.readUInt32BE(0), 23622);
  const junk = new Uint8Array([1, 2, 3]);
  assert.equal(setPngResolution(junk, 300), junk);
});

test("text size is kept in the link and clamped", () => {
  assert.equal(encodeState({ ...DEFAULTS, source: "x" }), "m=latex&s=x");
  assert.equal(decodeState("#t=24").state.textSize, 24);
  assert.equal(decodeState("#t=9999").state.textSize, 200);
  assert.equal(decodeState("#t=abc").state.textSize, 12);
});

test("styleSvg embeds MathML and links the description", () => {
  const mathml = '<math display="block"><mi>x</mi></math>';
  const a = styleSvg(RAW, { alt: "x", mathml });
  const b = styleSvg(RAW, { alt: "x", mathml });
  for (const svg of [a, b]) {
    assert.ok(svg.includes('xmlns="http://www.w3.org/1998/Math/MathML"'));
    assert.ok(svg.includes("<metadata>"));
    const title = svg.match(/<title id="([^"]+)"/)[1];
    const desc = svg.match(/<desc id="([^"]+)"/)[1];
    assert.ok(svg.includes(`aria-labelledby="${title}"`));
    assert.ok(svg.includes(`aria-describedby="${desc}"`));
    assert.ok(svg.indexOf("<title") < svg.indexOf("<desc"));
    assert.ok(svg.indexOf("<desc") < svg.indexOf("<metadata"));
    assert.ok(svg.indexOf("<metadata") < svg.indexOf("<g "));
  }
  // ids must differ, so several of these can be inlined into one document
  assert.notEqual(a.match(/<title id="([^"]+)"/)[1], b.match(/<title id="([^"]+)"/)[1]);
});

test("styleSvg without MathML keeps the plain label", () => {
  const svg = styleSvg(RAW, { alt: "x" });
  assert.ok(!svg.includes("<metadata>"));
  assert.ok(!svg.includes("aria-describedby"));
});

test("embedded MathML cannot break the SVG as XML", () => {
  const mathml = '<math><mo>&ApplyFunction;</mo><mi>a & b</mi><mo>&UnknownEntity;</mo></math>';
  const svg = styleSvg(RAW, { alt: "x", mathml });
  assert.ok(svg.includes("&#x2061;"), "named entity becomes a numeric reference");
  assert.ok(!svg.includes("&ApplyFunction;"));
  assert.ok(svg.includes("&amp;UnknownEntity;"), "unknown entities are escaped, not passed through");
  assert.ok(!/&(?!amp;|lt;|gt;|quot;|apos;|#)/.test(svg), "no undefined entities remain");
});

test("mathmlForXml keeps numeric and predefined entities", () => {
  assert.equal(mathmlForXml("<mi>&#x2061;&amp;&lt;</mi>"), "<mi>&#x2061;&amp;&lt;</mi>");
});
