// Checks the package as it will be published: a directory built by formel-wasm/build-wasm.sh
// --typescript with package.json and README.md copied in (see .github/workflows/npm.yml).
//
// Run with: node --test formel-wasm/npm/test/package.test.mjs
//
// The point is to exercise the shipped artifact, not the crate: the glue is the wasm-bindgen
// `--target web` output, the module is the wasm-opt'ed one, and the font is the test font from the
// repository (the package ships no font, the user supplies one).
import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const dist = new URL("../dist/", import.meta.url);
const at = (name) => new URL(name, dist);

const glue = await import(at("formel_wasm.js").href);
const wasm = await readFile(at("formel_wasm_bg.wasm"));
// Node's fetch cannot read `file:` URLs, so hand the glue the bytes wrapped in a Response: that is
// the streaming path a browser takes when the server sends `application/wasm`.
await glue.default(
  new Response(wasm, { headers: { "content-type": "application/wasm" } }),
);

const font = new Uint8Array(
  await readFile(new URL("../../../tests/testfiles/latinmodern-math.otf", import.meta.url)),
);

test("the module instantiates and renders MathML to SVG", () => {
  const renderer = new glue.MathRenderer(font);
  assert.ok(renderer.units_per_em() === 1000 || renderer.units_per_em() > 0);

  const rendering = renderer.render("<math><mfrac><mi>a</mi><mi>b</mi></mfrac></math>", false);
  assert.ok(rendering.svg.startsWith("<svg"), `not an SVG: ${rendering.svg.slice(0, 40)}`);
  assert.match(rendering.svg, /viewBox=/);
  assert.ok(rendering.width > 0 && rendering.height > 0);
  // the baseline is inside the box, which is what makes the result placeable inline
  assert.ok(rendering.baseline >= 0 && rendering.baseline <= rendering.height);

  assert.ok(renderer.render_svg("<math><mi>x</mi></math>").startsWith("<svg"));
});

test("LaTeX converts to MathML, and a syntax error reports its range", () => {
  const mathml = glue.latex_to_mathml("\\det \\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}", true);
  assert.match(mathml, /<mtable/);
  assert.match(mathml, /application\/x-tex/);

  // the README's "LaTeX in, SVG out" path
  const renderer = new glue.MathRenderer(font);
  assert.ok(renderer.render_svg(mathml).startsWith("<svg"));

  assert.equal(glue.latex_error_range("\\frac{a}{b}", true), undefined);
  // `\notacommand` is what the crate's own test uses as a certain error; the range is a typed array
  const range = Array.from(glue.latex_error_range("x + \\notacommand y", true) ?? []);
  assert.equal(range.length, 2, `expected a range, got ${range}`);
  // the command starts after "x + "
  assert.deepEqual(range, [4, 16]);
});

test("the tarball contains what the export map points at", async () => {
  const output = execFileSync("npm", ["pack", "--dry-run", "--json"], {
    cwd: fileURLToPath(dist),
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
  const parsed = JSON.parse(output);
  // npm >= 11 keys the report by package name, older versions return a one-element array
  const report = Array.isArray(parsed) ? parsed[0] : Object.values(parsed)[0];
  const packed = new Set(report.files.map((file) => file.path));

  for (const required of [
    "package.json",
    "README.md",
    "LICENSE",
    "THIRD_PARTY.md",
    "formel_wasm.js",
    "formel_wasm.d.ts",
    "formel_wasm_bg.wasm",
    "formel_wasm_bg.wasm.d.ts",
  ]) {
    assert.ok(packed.has(required), `${required} is missing from the published files`);
  }
  // npm shows the licence from the tarball; a package without one is a licence violation, not a style nit
  const manifest = JSON.parse(await readFile(at("package.json"), "utf8"));
  assert.equal(manifest.license, "MIT");
  assert.ok(packed.has("LICENSE"), "the MIT text has to travel with the binary");
  // nothing from the build directory leaks in
  for (const file of packed) {
    assert.ok(!file.includes("..") && !file.startsWith("/"), `unexpected path ${file}`);
  }
});
