// Rendering assertions against the built module. Run after web/build.sh:
//   node --test web/test/render.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import init, { MathRenderer, latex_to_mathml } from "../pkg/mathwasm.js";

await init({ module_or_path: fs.readFileSync(new URL("../pkg/mathwasm_bg.wasm", import.meta.url)) });
const renderer = new MathRenderer(fs.readFileSync(new URL("../fonts/newcomputermodern-math.otf", import.meta.url)));

/** Ink bounds of each glyph path, in font units, with its group transform applied. */
function paths(svg) {
  const out = [];
  for (const m of svg.matchAll(/<g transform="translate\(([-\d.]+),\s*([-\d.]+)\) scale\(([-\d.]+),\s*([-\d.]+)\)">([\s\S]*?)<\/g>/g)) {
    const [tx, ty, sx, sy] = m.slice(1, 5).map(Number);
    for (const d of m[5].matchAll(/d="([^"]+)"/g)) {
      const nums = d[1].match(/-?\d+\.?\d*/g).map(Number);
      const xs = nums.filter((_, i) => i % 2 === 0).map((x) => tx + sx * x);
      const ys = nums.filter((_, i) => i % 2 === 1).map((y) => ty + sy * y);
      out.push({ x0: Math.min(...xs), x1: Math.max(...xs), y0: Math.min(...ys), y1: Math.max(...ys) });
    }
  }
  return out;
}

/**
 * How much of the formula's width everything above the letters covers.
 *
 * The accent is identified by position, not by being the topmost path: an assembled arrow is
 * several pieces (cap, repeated bars, head) whose topmost piece is only the head.
 */
function accentCoverage(tex) {
  const ps = paths(renderer.render_svg(latex_to_mathml(tex, true)));
  const total = Math.max(...ps.map((p) => p.x1)) - Math.min(...ps.map((p) => p.x0));
  const tallest = ps.reduce((a, b) => (b.y1 - b.y0 > a.y1 - a.y0 ? b : a));
  const accent = ps.filter((p) => p.y1 <= tallest.y0);
  const width = Math.max(...accent.map((p) => p.x1)) - Math.min(...accent.map((p) => p.x0));
  return width / total;
}

test("over- and under-arrows stretch to their base", () => {
  for (const tex of [String.raw`\overrightarrow{ABCDEFGH}`, String.raw`\overleftarrow{ABCDEFGH}`]) {
    assert.ok(accentCoverage(tex) > 0.8, `${tex} accent covers the base (got ${accentCoverage(tex)})`);
  }
});

test("wide hats still use the font's accent variants", () => {
  assert.ok(accentCoverage(String.raw`\widehat{abc}`) > 0.8);
});
