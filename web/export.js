// Pure helpers for the demo page (no DOM access, so they can be tested with `node --test`).

export const DEFAULTS = Object.freeze({
  mode: "latex",
  source: "",
  font: "latinmodern-math.otf",
  display: true,
  fg: "#000000",
  bg: null, // null = transparent
  size: 3,
  debug: false,
});

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
export function styleSvg(rawSvg, { fg = DEFAULTS.fg, bg = null, alt = "" } = {}) {
  let svg = rawSvg.replaceAll('"black"', `"${fg}"`);
  const label = escapeXml(oneLine(alt));
  const open = svg.match(/^<svg\s([^>]*)>/);
  if (!open) return svg;

  let extra = "";
  if (label) extra += `<title>${label}</title>`;
  if (bg) {
    const box = open[1].match(/viewBox="([^"]+)"/);
    if (box) {
      const [x, y, w, h] = box[1].trim().split(/\s+/);
      extra += `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${bg}"/>`;
    }
  }
  const attrs = label ? `${open[1]} role="img" aria-label="${label}"` : open[1];
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
export function snippets({ svg, alt, display, width, height, baseline, unitsPerEm }) {
  const w = em(width / unitsPerEm);
  const h = em(height / unitsPerEm);
  const below = em((height - baseline) / unitsPerEm);
  const attrAlt = escapeXml(oneLine(alt));
  const position = display ? "display:block;margin:1em auto" : `vertical-align:-${below}`;
  const uri = svgDataUri(svg);

  // inline <svg> needs explicit sizes, otherwise browsers default to 300x150
  const inlineSvg = svg.replace("<svg ", `<svg width="${w}" height="${h}" style="${position}" `);

  return {
    img: `<img src="${uri}" alt="${attrAlt}" style="height:${h};${position}">`,
    svg: inlineSvg,
    markdownFile: `![${escapeMarkdownAlt(alt)}](formula.svg)`,
    markdownData: `![${escapeMarkdownAlt(alt)}](${uri})`,
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
  if (s.debug) p.set("x", "1");
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

  state.debug = p.get("x") === "1";
  return { state, hasSource };
}
