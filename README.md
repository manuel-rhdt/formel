# formel

[![crates.io](https://img.shields.io/crates/v/formel.svg)](https://crates.io/crates/formel)
[![docs.rs](https://docs.rs/formel/badge.svg)](https://docs.rs/formel)

A Rust library that lays out mathematical formulas using
**OpenType MATH fonts** and [rustybuzz](https://github.com/harfbuzz/rustybuzz) for unicode shaping.
It implements the typesetting rules from the OpenType MATH specification and
[MathML Core](https://www.w3.org/TR/mathml-core/): script and fraction placement, stretchy operators,
radicals, tables, accents, italic corrections, math kerning and more, comparable in quality to
modern unicode TeX engines, like LuaTeX and XeTeX. The output, i.e. the glyph positions and sizes, can be
directly consumed by users of the rust library. A simple SVG renderer is provided, too.

**[Try it in the browser →](https://manuel-rhdt.github.io/formel/)** Type LaTeX or MathML, compare the
bundled fonts (or drop in your own), and copy or download the result as SVG or PNG. The demo is this crate
compiled to WebAssembly (`formel-wasm/`, `web/`), resulting in a **small WebAssembly bundle of ~1 MB**.

## Gallery

Any math font that is compatible with LuaTeX or XeTeX (or MS Word) can be used with *formel*.

### Fractions, radicals & continued fractions

<table>
<tr><td width="150"><sub>New Computer Modern Math</sub></td><td><img alt="quadratic in New Computer Modern Math" src="docs/img/quadratic-newcm.svg" height="102"></td></tr>
<tr><td width="150"><sub>STIX Two Math</sub></td><td><img alt="quadratic in STIX Two Math" src="docs/img/quadratic-stix.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><img alt="quadratic in TeX Gyre Termes Math" src="docs/img/quadratic-termes.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><img alt="quadratic in TeX Gyre Pagella Math" src="docs/img/quadratic-pagella.svg" height="110"></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><img alt="quadratic in Asana Math" src="docs/img/quadratic-asana.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre Schola Math</sub></td><td><img alt="quadratic in TeX Gyre Schola Math" src="docs/img/quadratic-schola.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><img alt="quadratic in TeX Gyre Bonum Math" src="docs/img/quadratic-bonum.svg" height="110"></td></tr>
<tr><td width="150"><sub>Cambria Math</sub></td><td><img alt="quadratic in Cambria Math" src="docs/img/quadratic-cambria.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><img alt="quadratic in TeX Gyre DejaVu Math" src="docs/img/quadratic-dejavu.svg" height="110"></td></tr>
<tr><td width="150"><sub>Noto Sans Math</sub></td><td><img alt="quadratic in Noto Sans Math" src="docs/img/quadratic-noto.svg" height="110"></td></tr>
<tr><td width="150"><sub>Fira Math</sub></td><td><img alt="quadratic in Fira Math" src="docs/img/quadratic-fira.svg" height="110"></td></tr>
</table>

For comparison, the same formula as LaTeX, rendered live by GitHub (MathJax) in your browser:

$$
x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}, \qquad \varphi = 1 + \cfrac{1}{1 + \cfrac{1}{1 + \cfrac{1}{1 + \dots}}}
$$

<details>
<summary>MathML source</summary>

```xml
<math display="block">
  <mi>x</mi><mo>=</mo>
  <mfrac>
    <mrow><mo>&minus;</mo><mi>b</mi><mo>&pm;</mo>
      <msqrt><msup><mi>b</mi><mn>2</mn></msup><mo>&minus;</mo><mn>4</mn><mi>a</mi><mi>c</mi></msqrt>
    </mrow>
    <mrow><mn>2</mn><mi>a</mi></mrow>
  </mfrac>
  <mo>,</mo><mspace width="1em"/>
  <mi>&phi;</mi><mo>=</mo>
  <mn>1</mn><mo>+</mo>
  <mfrac><mn>1</mn><mrow><mn>1</mn><mo>+</mo><mfrac><mn>1</mn><mrow><mn>1</mn><mo>+</mo><mfrac><mn>1</mn><mrow><mn>1</mn><mo>+</mo><mo>&hellip;</mo></mrow></mfrac></mrow></mfrac></mrow></mfrac>
</math>
```

</details>

### Big operators with limits, integrals

Display-style `∑` is enlarged and gets under/over limits, while `∫` takes its limits as scripts and is italic-corrected.

<table>
<tr><td width="150"><sub>New Computer Modern Math</sub></td><td><img alt="basel in New Computer Modern Math" src="docs/img/basel-newcm.svg" height="103"></td></tr>
<tr><td width="150"><sub>STIX Two Math</sub></td><td><img alt="basel in STIX Two Math" src="docs/img/basel-stix.svg" height="108"></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><img alt="basel in TeX Gyre Termes Math" src="docs/img/basel-termes.svg" height="106"></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><img alt="basel in TeX Gyre Pagella Math" src="docs/img/basel-pagella.svg" height="101"></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><img alt="basel in Asana Math" src="docs/img/basel-asana.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre Schola Math</sub></td><td><img alt="basel in TeX Gyre Schola Math" src="docs/img/basel-schola.svg" height="104"></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><img alt="basel in TeX Gyre Bonum Math" src="docs/img/basel-bonum.svg" height="93"></td></tr>
<tr><td width="150"><sub>Cambria Math</sub></td><td><img alt="basel in Cambria Math" src="docs/img/basel-cambria.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><img alt="basel in TeX Gyre DejaVu Math" src="docs/img/basel-dejavu.svg" height="97"></td></tr>
<tr><td width="150"><sub>Noto Sans Math</sub></td><td><img alt="basel in Noto Sans Math" src="docs/img/basel-noto.svg" height="110"></td></tr>
<tr><td width="150"><sub>Fira Math</sub></td><td><img alt="basel in Fira Math" src="docs/img/basel-fira.svg" height="110"></td></tr>
</table>

For comparison, the same formula as LaTeX, rendered live by GitHub (MathJax) in your browser:

$$
\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}, \qquad \int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}
$$

<details>
<summary>MathML source</summary>

```xml
<math display="block">
  <munderover><mo>&Sum;</mo><mrow><mi>n</mi><mo>=</mo><mn>1</mn></mrow><mi>&infin;</mi></munderover>
  <mfrac><mn>1</mn><msup><mi>n</mi><mn>2</mn></msup></mfrac>
  <mo>=</mo>
  <mfrac><msup><mi>&pi;</mi><mn>2</mn></msup><mn>6</mn></mfrac>
  <mo>,</mo><mspace width="1em"/>
  <msubsup><mo>&int;</mo><mrow><mo>&minus;</mo><mi>&infin;</mi></mrow><mi>&infin;</mi></msubsup>
  <msup><mi>e</mi><mrow><mo>&minus;</mo><msup><mi>x</mi><mn>2</mn></msup></mrow></msup>
  <mi>d</mi><mi>x</mi>
  <mo>=</mo>
  <msqrt><mi>&pi;</mi></msqrt>
</math>
```

</details>

### Stretchy operators & bold vectors

Contour and surface integrals with the correct glyph variants, plus bold math alphanumerics.

<table>
<tr><td width="150"><sub>New Computer Modern Math</sub></td><td><img alt="stokes in New Computer Modern Math" src="docs/img/stokes-newcm.svg" height="110"></td></tr>
<tr><td width="150"><sub>STIX Two Math</sub></td><td><img alt="stokes in STIX Two Math" src="docs/img/stokes-stix.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><img alt="stokes in TeX Gyre Termes Math" src="docs/img/stokes-termes.svg" height="82"></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><img alt="stokes in TeX Gyre Pagella Math" src="docs/img/stokes-pagella.svg" height="97"></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><img alt="stokes in Asana Math" src="docs/img/stokes-asana.svg" height="86"></td></tr>
<tr><td width="150"><sub>TeX Gyre Schola Math</sub></td><td><img alt="stokes in TeX Gyre Schola Math" src="docs/img/stokes-schola.svg" height="75"></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><img alt="stokes in TeX Gyre Bonum Math" src="docs/img/stokes-bonum.svg" height="75"></td></tr>
<tr><td width="150"><sub>Cambria Math</sub></td><td><img alt="stokes in Cambria Math" src="docs/img/stokes-cambria.svg" height="110"></td></tr>
<tr><td width="150"><sub>Noto Sans Math</sub></td><td><img alt="stokes in Noto Sans Math" src="docs/img/stokes-noto.svg" height="110"></td></tr>
<tr><td width="150"><sub>Fira Math</sub></td><td><img alt="stokes in Fira Math" src="docs/img/stokes-fira.svg" height="110"></td></tr>
</table>

For comparison, the same formula as LaTeX, rendered live by GitHub (MathJax) in your browser:

$$
\oint_{\partial S} \mathbf{F}\cdot d\mathbf{s} = \iint_S \nabla\times\mathbf{F}\cdot d\mathbf{s}
$$

<details>
<summary>MathML source</summary>

```xml
<math display="block">
  <msub><mo>&conint;</mo><mrow><mi>&part;</mi><mi>S</mi></mrow></msub>
  <mi mathvariant="bold">F</mi><mo>&sdot;</mo><mrow><mi>d</mi><mi mathvariant="bold">s</mi></mrow>
  <mo>=</mo>
  <msub><mo>&Int;</mo><mi>S</mi></msub>
  <mo>&Del;</mo><mo>&times;</mo><mi mathvariant="bold">F</mi><mo>&sdot;</mo><mrow><mi>d</mi><mi mathvariant="bold">s</mi></mrow>
</math>
```

</details>

### Tables, stretchy delimiters & cases

`mtable` layout with ellipsis glyphs; parentheses and braces stretch to the height of the content via glyph variants and assemblies.

<table>
<tr><td width="150"><sub>New Computer Modern Math</sub></td><td><img alt="matrix in New Computer Modern Math" src="docs/img/matrix-newcm.svg" height="105"></td></tr>
<tr><td width="150"><sub>STIX Two Math</sub></td><td><img alt="matrix in STIX Two Math" src="docs/img/matrix-stix.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><img alt="matrix in TeX Gyre Termes Math" src="docs/img/matrix-termes.svg" height="95"></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><img alt="matrix in TeX Gyre Pagella Math" src="docs/img/matrix-pagella.svg" height="95"></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><img alt="matrix in Asana Math" src="docs/img/matrix-asana.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre Schola Math</sub></td><td><img alt="matrix in TeX Gyre Schola Math" src="docs/img/matrix-schola.svg" height="108"></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><img alt="matrix in TeX Gyre Bonum Math" src="docs/img/matrix-bonum.svg" height="87"></td></tr>
<tr><td width="150"><sub>Cambria Math</sub></td><td><img alt="matrix in Cambria Math" src="docs/img/matrix-cambria.svg" height="108"></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><img alt="matrix in TeX Gyre DejaVu Math" src="docs/img/matrix-dejavu.svg" height="89"></td></tr>
<tr><td width="150"><sub>Noto Sans Math</sub></td><td><img alt="matrix in Noto Sans Math" src="docs/img/matrix-noto.svg" height="110"></td></tr>
</table>

For comparison, the same formula as LaTeX, rendered live by GitHub (MathJax) in your browser:

$$
A = \begin{pmatrix} a_{11} & \cdots & a_{1n} \\ \vdots & \ddots & \vdots \\ a_{m1} & \cdots & a_{mn} \end{pmatrix}, \qquad |x| = \begin{cases} x & \text{if } x \ge 0 \\ -x & \text{if } x < 0 \end{cases}
$$

<details>
<summary>MathML source</summary>

```xml
<math display="block">
  <mi>A</mi><mo>=</mo>
  <mrow><mo>(</mo>
    <mtable>
      <mtr><mtd><msub><mi>a</mi><mn>11</mn></msub></mtd><mtd><mo>&hellip;</mo></mtd><mtd><msub><mi>a</mi><mrow><mn>1</mn><mi>n</mi></mrow></msub></mtd></mtr>
      <mtr><mtd><mo>&vellip;</mo></mtd><mtd><mo>&dtdot;</mo></mtd><mtd><mo>&vellip;</mo></mtd></mtr>
      <mtr><mtd><msub><mi>a</mi><mrow><mi>m</mi><mn>1</mn></mrow></msub></mtd><mtd><mo>&hellip;</mo></mtd><mtd><msub><mi>a</mi><mrow><mi>m</mi><mi>n</mi></mrow></msub></mtd></mtr>
    </mtable>
  <mo>)</mo></mrow>
  <mo>,</mo><mspace width="1em"/>
  <mrow><mo>|</mo><mi>x</mi><mo>|</mo></mrow><mo>=</mo>
  <mrow><mo>{</mo>
    <mtable columnalign="left">
      <mtr><mtd><mi>x</mi></mtd><mtd><mtext>if&#xA0;</mtext><mi>x</mi><mo>&ge;</mo><mn>0</mn></mtd></mtr>
      <mtr><mtd><mo>&minus;</mo><mi>x</mi></mtd><mtd><mtext>if&#xA0;</mtext><mi>x</mi><mo>&lt;</mo><mn>0</mn></mtd></mtr>
    </mtable>
  </mrow>
</math>
```

</details>

### Fractions, brackets, accents

Quantum-mechanics notation: stretchy brackets and bras/kets, `ħ`, a wide-hat accent, and the Laplacian.

<table>
<tr><td width="150"><sub>New Computer Modern Math</sub></td><td><img alt="schrodinger in New Computer Modern Math" src="docs/img/schrodinger-newcm.svg" height="80"></td></tr>
<tr><td width="150"><sub>STIX Two Math</sub></td><td><img alt="schrodinger in STIX Two Math" src="docs/img/schrodinger-stix.svg" height="81"></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><img alt="schrodinger in TeX Gyre Termes Math" src="docs/img/schrodinger-termes.svg" height="76"></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><img alt="schrodinger in TeX Gyre Pagella Math" src="docs/img/schrodinger-pagella.svg" height="81"></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><img alt="schrodinger in Asana Math" src="docs/img/schrodinger-asana.svg" height="110"></td></tr>
<tr><td width="150"><sub>TeX Gyre Schola Math</sub></td><td><img alt="schrodinger in TeX Gyre Schola Math" src="docs/img/schrodinger-schola.svg" height="71"></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><img alt="schrodinger in TeX Gyre Bonum Math" src="docs/img/schrodinger-bonum.svg" height="70"></td></tr>
<tr><td width="150"><sub>Cambria Math</sub></td><td><img alt="schrodinger in Cambria Math" src="docs/img/schrodinger-cambria.svg" height="82"></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><img alt="schrodinger in TeX Gyre DejaVu Math" src="docs/img/schrodinger-dejavu.svg" height="70"></td></tr>
<tr><td width="150"><sub>Noto Sans Math</sub></td><td><img alt="schrodinger in Noto Sans Math" src="docs/img/schrodinger-noto.svg" height="83"></td></tr>
<tr><td width="150"><sub>Fira Math</sub></td><td><img alt="schrodinger in Fira Math" src="docs/img/schrodinger-fira.svg" height="78"></td></tr>
</table>

For comparison, the same formula as LaTeX, rendered live by GitHub (MathJax) in your browser:

$$
i\hbar\frac{\partial}{\partial t}\Psi = \left[-\frac{\hbar^2}{2m}\nabla^2 + V\right]\Psi, \qquad Z = \langle\psi|\,e^{-\beta\hat H}\,|\psi\rangle
$$

<details>
<summary>MathML source</summary>

```xml
<math display="block">
  <mi>i</mi><mi>&hbar;</mi>
  <mfrac><mo>&part;</mo><mrow><mo>&part;</mo><mi>t</mi></mrow></mfrac>
  <mi>&Psi;</mi><mo>=</mo>
  <mrow><mo>[</mo>
    <mo>&minus;</mo>
    <mfrac><msup><mi>&hbar;</mi><mn>2</mn></msup><mrow><mn>2</mn><mi>m</mi></mrow></mfrac>
    <msup><mo>&nabla;</mo><mn>2</mn></msup><mo>+</mo><mi>V</mi>
  <mo>]</mo></mrow>
  <mi>&Psi;</mi>
  <mo>,</mo><mspace width="1em"/>
  <mi>Z</mi><mo>=</mo>
  <mrow><mo stretchy="false">&langle;</mo><mi>&psi;</mi><mo stretchy="false">|</mo><msup><mi>e</mi><mrow><mo>&minus;</mo><mi>&beta;</mi><mover accent="true"><mi>H</mi><mo stretchy="false">&#x302;</mo></mover></mrow></msup><mo stretchy="false">|</mo><mi>&psi;</mi><mo stretchy="false">&rangle;</mo></mrow>
</math>
```

</details>

### Prescripts (`mmultiscripts`)

Stacked pre-super/subscripts as used for isotopes and tensor indices.

<table>
<tr><td width="150"><sub>New Computer Modern Math</sub></td><td><img alt="nuclear in New Computer Modern Math" src="docs/img/nuclear-newcm.svg" height="44"></td></tr>
<tr><td width="150"><sub>STIX Two Math</sub></td><td><img alt="nuclear in STIX Two Math" src="docs/img/nuclear-stix.svg" height="48"></td></tr>
<tr><td width="150"><sub>TeX Gyre Termes Math</sub></td><td><img alt="nuclear in TeX Gyre Termes Math" src="docs/img/nuclear-termes.svg" height="47"></td></tr>
<tr><td width="150"><sub>TeX Gyre Pagella Math</sub></td><td><img alt="nuclear in TeX Gyre Pagella Math" src="docs/img/nuclear-pagella.svg" height="49"></td></tr>
<tr><td width="150"><sub>Asana Math</sub></td><td><img alt="nuclear in Asana Math" src="docs/img/nuclear-asana.svg" height="51"></td></tr>
<tr><td width="150"><sub>TeX Gyre Schola Math</sub></td><td><img alt="nuclear in TeX Gyre Schola Math" src="docs/img/nuclear-schola.svg" height="49"></td></tr>
<tr><td width="150"><sub>TeX Gyre Bonum Math</sub></td><td><img alt="nuclear in TeX Gyre Bonum Math" src="docs/img/nuclear-bonum.svg" height="43"></td></tr>
<tr><td width="150"><sub>Cambria Math</sub></td><td><img alt="nuclear in Cambria Math" src="docs/img/nuclear-cambria.svg" height="46"></td></tr>
<tr><td width="150"><sub>TeX Gyre DejaVu Math</sub></td><td><img alt="nuclear in TeX Gyre DejaVu Math" src="docs/img/nuclear-dejavu.svg" height="45"></td></tr>
<tr><td width="150"><sub>Noto Sans Math</sub></td><td><img alt="nuclear in Noto Sans Math" src="docs/img/nuclear-noto.svg" height="48"></td></tr>
<tr><td width="150"><sub>Fira Math</sub></td><td><img alt="nuclear in Fira Math" src="docs/img/nuclear-fira.svg" height="53"></td></tr>
</table>

For comparison, the same formula as LaTeX, rendered live by GitHub (MathJax) in your browser:

$$
{}^{235}_{\;\,92}\mathrm{U} + n \to {}^{141}_{\;\,56}\mathrm{Ba} + {}^{92}_{\;\,36}\mathrm{Kr} + 3n, \qquad {}^{\alpha}_{\beta}\Lambda^{\mu}_{\nu}
$$

<details>
<summary>MathML source</summary>

```xml
<math display="block"><mmultiscripts><mi mathvariant="normal">U</mi><mprescripts/><mn>92</mn><mn>235</mn></mmultiscripts><mo>+</mo><mi>n</mi><mo>&rarr;</mo><mmultiscripts><mi>Ba</mi><mprescripts/><mn>56</mn><mn>141</mn></mmultiscripts><mo>+</mo><mmultiscripts><mi>Kr</mi><mprescripts/><mn>36</mn><mn>92</mn></mmultiscripts><mo>+</mo><mn>3</mn><mi>n</mi><mo>,</mo><mspace width="1em"/><mmultiscripts><mi>&Lambda;</mi><mi>&nu;</mi><mi>&mu;</mi><mprescripts/><mi>&beta;</mi><mi>&alpha;</mi></mmultiscripts></math>
```

</details>

## License

See [LICENSE](LICENSE).
