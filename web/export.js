// Pure helpers for the demo page (no DOM access, so they can be tested with `node --test`).

export const DEFAULTS = Object.freeze({
  mode: "latex",
  source: "",
  font: "newcomputermodern-math.otf",
  display: true,
  fg: "#000000",
  bg: null, // null = transparent
  size: 3,
  textSize: 12, // pt: the formula's em matches text of this size
});

/** Text sizes that may be chosen, in pt. */
export const TEXT_SIZE_RANGE = Object.freeze({ min: 4, max: 200 });

const HEX_COLOR = /^#[0-9a-f]{6}$/i;

/** Escapes text for use in XML/HTML text and double-quoted attributes. */
export function escapeXml(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Collapses whitespace so that the text fits on one line (alt texts, titles). */
export function oneLine(text) {
  return String(text).replace(/\s+/g, " ").trim();
}

/**
 * Applies the chosen colors and adds an accessible name to an SVG produced by mathwasm.
 *
 * The renderer paints everything in `black`, so recoloring is a replacement of that keyword.
 * `bg` of `null` keeps the background transparent.
 */
let svgIdCounter = 0;

const XML_ENTITIES = new Set(["amp;", "lt;", "gt;", "quot;", "apos;"]);
// The HTML named entities math-core emits, as codepoints. Only amp/lt/gt/quot/apos exist in XML.
const HTML_ENTITIES = { ApplyFunction: 0x2061, InvisibleTimes: 0x2062, InvisibleComma: 0x2063, nbsp: 0xa0 };

/**
 * Makes MathML safe to embed in an XML document. math-core writes HTML named entities such as
 * `&ApplyFunction;`, which are undefined in XML and make an SVG image fail to load; they become
 * numeric character references. Anything unrecognised is escaped rather than left to break the file.
 */
export function mathmlForXml(mathml) {
  return String(mathml).replace(/&([A-Za-z][A-Za-z0-9]+;|#x[0-9a-fA-F]+;|#\d+;)?/g, (match, entity) => {
    if (!entity) return "&amp;";
    if (entity[0] === "#" || XML_ENTITIES.has(entity)) return match;
    const name = entity.slice(0, -1);
    if (name in HTML_ENTITIES) return `&#x${HTML_ENTITIES[name].toString(16)};`;
    return `&amp;${entity}`;
  });
}

/**
 * Applies the chosen colors and makes an SVG produced by mathwasm self-describing.
 *
 * The renderer paints everything in `black`, so recoloring is a replacement of that keyword.
 * `bg` of `null` keeps the background transparent.
 *
 * `alt` becomes the title (the accessible name). With `mathml`, the SVG also gets a `desc`
 * pointing at the embedded MathML in `<metadata>`: an image cannot expose a formula's structure
 * to a screen reader, but the file can still carry the expression for tools and for assistive
 * technology that understands MathML. Ids are unique per call, so several of these SVGs can be
 * inlined into one document.
 */
export function styleSvg(rawSvg, { fg = DEFAULTS.fg, bg = null, alt = "", mathml = "" } = {}) {
  let svg = rawSvg.replaceAll('"black"', `"${fg}"`);
  const label = escapeXml(oneLine(alt));
  const open = svg.match(/^<svg\s([^>]*)>/);
  if (!open) return svg;

  // The renderer writes its own `<title>` when it was given an accessible name. Keep that one: a
  // second title would override it, and its ids are per document while the caller's are per call.
  const titled = /<title[\s>]/.test(svg);

  const id = `m${++svgIdCounter}`;
  let extra = "";
  if (label && !titled) extra += `<title id="${id}t">${label}</title>`;
  let labelled = "";
  if (mathml) {
    extra += `<desc id="${id}d">Vector rendering of a formula. The MathML it was typeset from is in the metadata element.</desc>`;
    extra += `<metadata>${mathmlForXml(mathml).replace(/^<math\b/, '<math xmlns="http://www.w3.org/1998/Math/MathML"')}</metadata>`;
    labelled = titled
      ? ` aria-describedby="${id}d"`
      : ` aria-labelledby="${id}t" aria-describedby="${id}d"`;
  }
  if (bg) {
    const box = open[1].match(/viewBox="([^"]+)"/);
    if (box) {
      const [x, y, w, h] = box[1].trim().split(/\s+/);
      extra += `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${bg}"/>`;
    }
  }
  const attrs = label && !titled
    ? `${open[1]} role="img" aria-label="${label}"${labelled}`
    : labelled
      ? `${open[1]}${labelled}`
      : open[1];
  return `<svg ${attrs}>${extra}${svg.slice(open[0].length)}`;
}

/** The parsed `viewBox` of an SVG string, or null. */
export function viewBox(svg) {
  const m = svg.match(/viewBox="([^"]+)"/);
  if (!m) return null;
  const [x, y, width, height] = m[1].trim().split(/\s+/).map(Number);
  return { x, y, width, height };
}

/**
 * Gives the SVG an absolute size, so that it works on its own (the renderer's SVG only has a
 * `viewBox`). `pxPerEm` is the size of one em of the font; `unitsPerEm` converts font units.
 */
export function sizeSvg(svg, pxPerEm, unitsPerEm) {
  const box = viewBox(svg);
  if (!box) return { svg, width: 0, height: 0 };
  const scale = pxPerEm / unitsPerEm;
  const width = box.width * scale;
  const height = box.height * scale;
  const sized = svg.replace("<svg ", `<svg width="${width.toFixed(2)}" height="${height.toFixed(2)}" `);
  return { svg: `<?xml version="1.0" encoding="UTF-8"?>\n${sized}\n`, width, height };
}

/**
 * The SVG as a standalone file whose width and height are in pt, so that a formula for
 * `textSize`-pt text is inserted at exactly that size by apps that honor SVG units. pt rather
 * than px: px is 1/96 in in CSS, but some apps take it as 1/72 in.
 */
export function svgInPoints(svg, textSize, unitsPerEm) {
  const box = viewBox(svg);
  if (!box) return { svg, widthPt: 0, heightPt: 0 };
  const scale = textSize / unitsPerEm;
  const widthPt = box.width * scale;
  const heightPt = box.height * scale;
  const sized = svg.replace("<svg ", `<svg width="${widthPt.toFixed(3)}pt" height="${heightPt.toFixed(3)}pt" `);
  return { svg: `<?xml version="1.0" encoding="UTF-8"?>\n${sized}\n`, widthPt, heightPt };
}

/** Pixels per em for a PNG of a formula for `textSize`-pt text at `dpi`. */
export function pixelsPerEm(textSize, dpi) {
  return (textSize / 72) * dpi;
}

let crcTable = null;
function crc32(bytes) {
  if (!crcTable) {
    crcTable = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      crcTable[n] = c >>> 0;
    }
  }
  let c = 0xffffffff;
  for (const b of bytes) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/**
 * Tags PNG bytes with a resolution (a `pHYs` chunk right after `IHDR`, replacing any existing
 * one), so that apps which read it insert the image at its intended physical size. Canvas
 * encoders write no resolution. Returns the input unchanged if it is not a PNG.
 */
export function setPngResolution(png, dpi) {
  const bytes = png instanceof Uint8Array ? png : new Uint8Array(png);
  const SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.length < 33 || SIGNATURE.some((b, i) => bytes[i] !== b)) return bytes;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const type = (at) => String.fromCharCode(...bytes.subarray(at + 4, at + 8));
  if (type(8) !== "IHDR") return bytes;

  const ppm = Math.round(dpi / 0.0254);   // pixels per metre
  const chunk = new Uint8Array(21);
  const cv = new DataView(chunk.buffer);
  cv.setUint32(0, 9);
  chunk.set([0x70, 0x48, 0x59, 0x73], 4);   // "pHYs"
  cv.setUint32(8, ppm);
  cv.setUint32(12, ppm);
  chunk[16] = 1;                             // unit: metre
  cv.setUint32(17, crc32(chunk.subarray(4, 17)));

  // copy the chunks, dropping any existing pHYs and inserting ours after IHDR
  const parts = [bytes.subarray(0, 8)];
  let at = 8;
  while (at + 12 <= bytes.length) {
    const length = view.getUint32(at);
    const end = at + 12 + length;
    if (end > bytes.length) return bytes;    // damaged; leave it alone
    if (type(at) !== "pHYs") parts.push(bytes.subarray(at, end));
    if (type(at) === "IHDR") parts.push(chunk);
    at = end;
  }
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let offset = 0;
  for (const p of parts) { out.set(p, offset); offset += p.length; }
  return out;
}

/** Base64 of the UTF-8 encoding of `text`. */
export function toBase64(text) {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

export function svgDataUri(svg) {
  return `data:image/svg+xml;base64,${toBase64(svg)}`;
}

function em(value) {
  // up to 3 decimals, without trailing zeros
  return `${Number(value.toFixed(3))}em`;
}

/** Escapes text for the alt part of a Markdown image. */
export function escapeMarkdownAlt(text) {
  return oneLine(text).replace(/([\\\[\]])/g, "\\$1");
}

/**
 * Snippets for embedding a rendering in other documents.
 *
 * @param {object} p
 * @param {string} p.svg        styled SVG (see `styleSvg`)
 * @param {string} [p.svgFile]  the SVG with an absolute size (see `sizeSvg`), for Markdown, which
 *                              shows images at their own size; defaults to `svg`
 * @param {string} [p.png]      data: URI of a PNG of the same image; if given, the snippets use
 *                              it instead of the SVG, and there is no inline-SVG snippet
 * @param {string} p.alt        alternative text, e.g. the LaTeX source
 * @param {boolean} p.display   block formula (centered) rather than inline
 * @param {number} p.width      viewBox width in font units
 * @param {number} p.height     viewBox height in font units
 * @param {number} p.baseline   distance from the top of the viewBox to the baseline
 * @param {number} p.unitsPerEm font units per em
 *
 * Sizes are in em, so the formula is as big as the surrounding text. For inline formulas the
 * image is shifted down by the part of the box below the baseline, so it lines up with the text.
 */
export function snippets({ svg, svgFile = svg, png = null, alt, display, width, height, baseline, unitsPerEm }) {
  const w = em(width / unitsPerEm);
  const h = em(height / unitsPerEm);
  const below = em((height - baseline) / unitsPerEm);
  const attrAlt = escapeXml(oneLine(alt));
  const position = display ? "display:block;margin:1em auto" : `vertical-align:-${below}`;
  const uri = png ?? svgDataUri(svg);
  const file = png ? "formula.png" : "formula.svg";

  // inline <svg> needs explicit sizes, otherwise browsers default to 300x150
  const inlineSvg = png ? null : svg.replace("<svg ", `<svg width="${w}" height="${h}" style="${position}" `);

  return {
    img: `<img src="${uri}" alt="${attrAlt}" style="height:${h};${position}">`,
    svg: inlineSvg,
    markdownFile: `![${escapeMarkdownAlt(alt)}](${file})`,
    markdownData: `![${escapeMarkdownAlt(alt)}](${png ?? svgDataUri(svgFile)})`,
  };
}

/** Serializes the page state into a URL fragment (without the `#`). Defaults are omitted. */
export function encodeState(state) {
  const s = { ...DEFAULTS, ...state };
  const p = new URLSearchParams();
  p.set("m", s.mode);
  p.set("s", s.source);
  if (s.font !== DEFAULTS.font) p.set("f", s.font);
  if (s.display !== DEFAULTS.display) p.set("d", s.display ? "1" : "0");
  if (s.fg.toLowerCase() !== DEFAULTS.fg) p.set("c", s.fg.slice(1).toLowerCase());
  if (s.bg) p.set("b", s.bg.slice(1).toLowerCase());
  if (s.size !== DEFAULTS.size) p.set("z", String(s.size));
  if (s.textSize !== DEFAULTS.textSize) p.set("t", String(s.textSize));
  return p.toString();
}

/**
 * Parses a URL fragment (with or without the `#`) back into a state. Unknown or invalid
 * values fall back to the defaults, so a damaged link still opens.
 *
 * @param {string} hash
 * @param {string[]} knownFonts font files that may be selected
 */
export function decodeState(hash, knownFonts = []) {
  const p = new URLSearchParams(hash.replace(/^#/, ""));
  const state = { ...DEFAULTS };
  const hasSource = p.has("s");

  const mode = p.get("m");
  if (mode === "latex" || mode === "mathml") state.mode = mode;
  if (hasSource) state.source = p.get("s");

  const font = p.get("f");
  if (font && knownFonts.includes(font)) state.font = font;

  const display = p.get("d");
  if (display === "0" || display === "1") state.display = display === "1";

  const fg = p.get("c");
  if (fg && HEX_COLOR.test(`#${fg}`)) state.fg = `#${fg.toLowerCase()}`;
  const bg = p.get("b");
  if (bg && HEX_COLOR.test(`#${bg}`)) state.bg = `#${bg.toLowerCase()}`;

  const size = Number(p.get("z"));
  if (p.has("z") && Number.isFinite(size)) state.size = Math.min(8, Math.max(1, size));

  const textSize = Number(p.get("t"));
  if (p.has("t") && Number.isFinite(textSize) && textSize > 0) {
    state.textSize = Math.min(TEXT_SIZE_RANGE.max, Math.max(TEXT_SIZE_RANGE.min, textSize));
  }
  return { state, hasSource };
}

/**
 * An HTML `<img>` for transferring an image to rich-text apps (drag and drop, clipboard).
 * `width` and `height` are in CSS px and set the size the image gets when inserted.
 */
export function htmlImage({ src, alt = "", width, height }) {
  const size = width && height ? ` width="${Math.round(width)}" height="${Math.round(height)}"` : "";
  return `<img src="${src}" alt="${escapeXml(oneLine(alt))}"${size}>`;
}
