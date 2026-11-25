// Type declarations for `dom.js`, which is plain JavaScript: `@manuel-rhdt/formel-wasm/dom`.

/** The parsed `viewBox` of an SVG. */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface StyleOptions {
  /** Colour of everything the renderer painted `black`, as a CSS colour. Default `"#000000"`. */
  fg?: string;
  /** Background fill, or `null` (the default) for a transparent one. */
  bg?: string | null;
  /** Accessible name. Becomes the `<title>` unless the SVG already has one. */
  alt?: string;
  /** The formula's MathML; adds a `<desc>` and embeds it in `<metadata>`. */
  mathml?: string;
}

export interface SnippetInput {
  /** SVG for inline use, e.g. from `styleSvg`. */
  svg: string;
  /** SVG for an `<img>`, which cannot inherit the text colour. Defaults to `svg`. */
  imgSvg?: string;
  mathml?: string;
  alt: string;
  /** Block formula rather than inline. */
  display: boolean;
  /** `viewBox` width in font units. */
  width: number;
  /** `viewBox` height in font units. */
  height: number;
  /** Distance from the top of the `viewBox` to the baseline, in font units. */
  baseline: number;
  /** Font units per em, from `MathRenderer.units_per_em()`. */
  unitsPerEm: number;
}

/** Escapes text for use in XML/HTML text and double-quoted attributes. */
export function escapeXml(text: string): string;

/** Collapses whitespace so that the text fits on one line (alt texts, titles). */
export function oneLine(text: string): string;

/** MathML that is safe to embed in XML: numeric entities for the invisible operators. */
export function mathmlForXml(mathml: string): string;

/** Sets the colour, background, accessible name and optionally the source MathML. */
export function styleSvg(rawSvg: string, options?: StyleOptions): string;

/** The parsed `viewBox`, or `null` if the SVG has none. */
export function viewBox(svg: string): Box | null;

/** Gives the SVG an absolute size in CSS px, and returns that size. */
export function sizeSvg(
  svg: string,
  pxPerEm: number,
  unitsPerEm: number,
): { svg: string; width: number; height: number };

/** Gives the SVG a size in pt, so apps that honour SVG units insert it at the text's size. */
export function svgInPoints(
  svg: string,
  textSize: number,
  unitsPerEm: number,
): { svg: string; widthPt: number; heightPt: number };

/** Pixels per em for a PNG of a formula for `textSize`-pt text at `dpi`. */
export function pixelsPerEm(textSize: number, dpi: number): number;

/** Tags PNG bytes with a resolution (`pHYs` chunk). Non-PNG input is returned unchanged. */
export function setPngResolution(png: Uint8Array | ArrayBuffer, dpi: number): Uint8Array;

/** Base64 of the UTF-8 encoding of `text`. */
export function toBase64(text: string): string;

/** An SVG as a base64 `data:` URI. */
export function svgDataUri(svg: string): string;

/** An SVG as a percent-encoded `data:` URI, for use in an HTML attribute. */
export function svgDataUriEncoded(svg: string): string;

/** Markup for embedding a rendering: inline SVG, `<img>`, and the MathML as-is. */
export function snippets(input: SnippetInput): { svg: string; img: string; mathml: string };

/** An `<img>` for transferring an image to rich-text apps. Sizes are CSS px. */
export function htmlImage(input: {
  src: string;
  alt?: string;
  width?: number;
  height?: number;
}): string;
