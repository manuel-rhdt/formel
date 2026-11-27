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
