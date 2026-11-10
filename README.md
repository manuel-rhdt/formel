# math-render

A Rust library that lays out mathematical formulas (MathML in, positioned glyphs out) using the
**OpenType `MATH` table** of a font and [HarfBuzz](https://harfbuzz.github.io/) for shaping.
It implements the typesetting rules from the OpenType MATH specification and
[MathML Core](https://www.w3.org/TR/mathml-core/): script and fraction placement, stretchy operators,
radicals, tables, accents, italic corrections, math kerning and more, all driven by the metrics the font
provides, so a well-made math font gives you good output without any hand-tuned constants.

## Gallery

Every formula below is laid out by this crate and rendered to SVG with the bundled `mathimg` tool,
once per font. The only thing that changes between the renderings is the font.

### Fractions, radicals & continued fractions

Nested fractions shrink through script styles, and the radical's surd and rule are built from the font's `MATH` table variants and assemblies.

<table>
<tr><td width="150"><sub>Latin Modern Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/quadratic-latinmodern-dark.svg"><img alt="quadratic in Latin Modern Math" src="docs/img/quadratic-latinmodern-light.svg" height="102"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/quadratic-pagella-dark.svg"><img alt="quadratic in TeX Gyre Pagella Math" src="docs/img/quadratic-pagella-light.svg" height="110"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/quadratic-termes-dark.svg"><img alt="quadratic in TeX Gyre Termes Math" src="docs/img/quadratic-termes-light.svg" height="110"></picture></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/quadratic-asana-dark.svg"><img alt="quadratic in Asana Math" src="docs/img/quadratic-asana-light.svg" height="110"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/quadratic-bonum-dark.svg"><img alt="quadratic in TeX Gyre Bonum Math" src="docs/img/quadratic-bonum-light.svg" height="110"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/quadratic-dejavu-dark.svg"><img alt="quadratic in TeX Gyre DejaVu Math" src="docs/img/quadratic-dejavu-light.svg" height="110"></picture></td></tr>
</table>

### Big operators with limits, integrals

Display-style `∑` is enlarged and gets under/over limits, while `∫` takes its limits as scripts and is italic-corrected.

<table>
<tr><td width="150"><sub>Latin Modern Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/basel-latinmodern-dark.svg"><img alt="basel in Latin Modern Math" src="docs/img/basel-latinmodern-light.svg" height="103"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/basel-pagella-dark.svg"><img alt="basel in TeX Gyre Pagella Math" src="docs/img/basel-pagella-light.svg" height="101"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/basel-termes-dark.svg"><img alt="basel in TeX Gyre Termes Math" src="docs/img/basel-termes-light.svg" height="106"></picture></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/basel-asana-dark.svg"><img alt="basel in Asana Math" src="docs/img/basel-asana-light.svg" height="110"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/basel-bonum-dark.svg"><img alt="basel in TeX Gyre Bonum Math" src="docs/img/basel-bonum-light.svg" height="93"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/basel-dejavu-dark.svg"><img alt="basel in TeX Gyre DejaVu Math" src="docs/img/basel-dejavu-light.svg" height="97"></picture></td></tr>
</table>

### Stretchy operators & bold vectors

Contour and surface integrals with the correct glyph variants, plus bold math alphanumerics.

<table>
<tr><td width="150"><sub>Latin Modern Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/stokes-latinmodern-dark.svg"><img alt="stokes in Latin Modern Math" src="docs/img/stokes-latinmodern-light.svg" height="110"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/stokes-pagella-dark.svg"><img alt="stokes in TeX Gyre Pagella Math" src="docs/img/stokes-pagella-light.svg" height="97"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/stokes-termes-dark.svg"><img alt="stokes in TeX Gyre Termes Math" src="docs/img/stokes-termes-light.svg" height="82"></picture></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/stokes-asana-dark.svg"><img alt="stokes in Asana Math" src="docs/img/stokes-asana-light.svg" height="86"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/stokes-bonum-dark.svg"><img alt="stokes in TeX Gyre Bonum Math" src="docs/img/stokes-bonum-light.svg" height="75"></picture></td></tr>
</table>

### Tables, stretchy delimiters & cases

`mtable` layout with ellipsis glyphs; parentheses and braces stretch to the height of the content via glyph variants and assemblies.

<table>
<tr><td width="150"><sub>Latin Modern Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/matrix-latinmodern-dark.svg"><img alt="matrix in Latin Modern Math" src="docs/img/matrix-latinmodern-light.svg" height="85"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/matrix-pagella-dark.svg"><img alt="matrix in TeX Gyre Pagella Math" src="docs/img/matrix-pagella-light.svg" height="95"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/matrix-termes-dark.svg"><img alt="matrix in TeX Gyre Termes Math" src="docs/img/matrix-termes-light.svg" height="95"></picture></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/matrix-asana-dark.svg"><img alt="matrix in Asana Math" src="docs/img/matrix-asana-light.svg" height="110"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/matrix-bonum-dark.svg"><img alt="matrix in TeX Gyre Bonum Math" src="docs/img/matrix-bonum-light.svg" height="87"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/matrix-dejavu-dark.svg"><img alt="matrix in TeX Gyre DejaVu Math" src="docs/img/matrix-dejavu-light.svg" height="89"></picture></td></tr>
</table>

### Fractions, brackets, accents

Quantum-mechanics notation: stretchy brackets and bras/kets, `ħ`, a wide-hat accent, and the Laplacian.

<table>
<tr><td width="150"><sub>Latin Modern Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/schrodinger-latinmodern-dark.svg"><img alt="schrodinger in Latin Modern Math" src="docs/img/schrodinger-latinmodern-light.svg" height="78"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/schrodinger-pagella-dark.svg"><img alt="schrodinger in TeX Gyre Pagella Math" src="docs/img/schrodinger-pagella-light.svg" height="81"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/schrodinger-termes-dark.svg"><img alt="schrodinger in TeX Gyre Termes Math" src="docs/img/schrodinger-termes-light.svg" height="76"></picture></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/schrodinger-asana-dark.svg"><img alt="schrodinger in Asana Math" src="docs/img/schrodinger-asana-light.svg" height="110"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/schrodinger-bonum-dark.svg"><img alt="schrodinger in TeX Gyre Bonum Math" src="docs/img/schrodinger-bonum-light.svg" height="70"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/schrodinger-dejavu-dark.svg"><img alt="schrodinger in TeX Gyre DejaVu Math" src="docs/img/schrodinger-dejavu-light.svg" height="70"></picture></td></tr>
</table>

### Prescripts (`mmultiscripts`)

Stacked pre-super/subscripts as used for isotopes and tensor indices.

<table>
<tr><td width="150"><sub>Latin Modern Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/nuclear-latinmodern-dark.svg"><img alt="nuclear in Latin Modern Math" src="docs/img/nuclear-latinmodern-light.svg" height="44"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/nuclear-pagella-dark.svg"><img alt="nuclear in TeX Gyre Pagella Math" src="docs/img/nuclear-pagella-light.svg" height="49"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/nuclear-termes-dark.svg"><img alt="nuclear in TeX Gyre Termes Math" src="docs/img/nuclear-termes-light.svg" height="47"></picture></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/nuclear-asana-dark.svg"><img alt="nuclear in Asana Math" src="docs/img/nuclear-asana-light.svg" height="51"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/nuclear-bonum-dark.svg"><img alt="nuclear in TeX Gyre Bonum Math" src="docs/img/nuclear-bonum-light.svg" height="43"></picture></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/nuclear-dejavu-dark.svg"><img alt="nuclear in TeX Gyre DejaVu Math" src="docs/img/nuclear-dejavu-light.svg" height="45"></picture></td></tr>
</table>

## License

See [LICENSE](LICENSE).
