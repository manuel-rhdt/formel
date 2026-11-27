#!/usr/bin/env bash
# Regenerates stixtwo-math-subset.otf: a few glyphs of STIX Two Math with their MATH table entries
# (math kerning, italic correction, top accent attachment, variants) and the `ssty` GSUB feature.
# STIX Two Math is licensed under the SIL Open Font License 1.1 (see web/fonts/OFL-1.1.txt).
# Needs fontTools (pip install fonttools). Run from the repository root.
set -euo pipefail
pyftsubset web/fonts/stixtwo-math.otf \
  --unicodes=U+0020,U+002B,U+0041,U+0045,U+0046,U+0054,U+0056,U+0078,U+222B,U+1D447,U+1D449,U+1D453 \
  --layout-features=ssty \
  --no-hinting --notdef-outline \
  --output-file=tests/testfiles/stixtwo-math-subset.otf

# stixtwo-math-subset-hanging.otf: the same font with the three parts of the integral's assembly
# drawn below their origin instead of above it (ink from -h+11 to 11, like the parentheses of
# Asana Math 0.962). MathML Core draws a vertical part by its bottom edge, so both fonts must give
# the same assembly. Only the outlines change; the MATH table and the advances do not.
python3 - <<'PY'
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

font = TTFont("tests/testfiles/stixtwo-math-subset.otf")
glyphs = font.getGlyphSet()
cff = font["CFF "].cff
top = cff.topDictIndex[0]
for name in ("uni2321", "integral.x", "uni2320"):
    bounds = BoundsPen(glyphs)
    glyphs[name].draw(bounds)
    pen = T2CharStringPen(glyphs[name].width, glyphs)
    glyphs[name].draw(TransformPen(pen, (1, 0, 0, 1, 0, 11 - bounds.bounds[3])))
    top.CharStrings[name] = pen.getCharString(private=top.Private, globalSubrs=cff.GlobalSubrs)
font.save("tests/testfiles/stixtwo-math-subset-hanging.otf")
PY
