// The demo page's state: what the URL fragment carries and what the controls default to.
// The embedding helpers live in `dom.js`, which is also the package's `./dom` entry point.

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

export * from "./dom.js";

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

