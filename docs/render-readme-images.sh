#!/usr/bin/env bash
# Renders docs/equations/*.xml with several OpenType math fonts into docs/img/
# (set MATHIMG=path to use a prebuilt binary, ONLY=font to render a single font)
# (a light and a dark variant of each, for use with <picture> in the README).
set -euo pipefail
cd "$(dirname "$0")/.."

declare -A FONTS=(
  [newcm]=${NEWCM_MATH:-$HOME/.local/share/fonts/NewCMMath-Regular.otf}
  [pagella]=/usr/share/texmf/fonts/opentype/public/tex-gyre-math/texgyrepagella-math.otf
  [termes]=/usr/share/texmf/fonts/opentype/public/tex-gyre-math/texgyretermes-math.otf
  [asana]=/usr/share/fonts/opentype/asana-math/Asana-Math.otf
  [dejavu]=/usr/share/fonts/truetype/dejavu/DejaVuMathTeXGyre.ttf
  [bonum]=/usr/share/texmf/fonts/opentype/public/tex-gyre-math/texgyrebonum-math.otf
  [schola]=/usr/share/texmf/fonts/opentype/public/tex-gyre-math/texgyreschola-math.otf
  [fira]=${FIRA_MATH:-$HOME/.local/share/fonts/FiraMath-Regular.otf}
  [noto]=${NOTO_SANS_MATH:-$HOME/.local/share/fonts/NotoSansMath-Math.ttf}
  [cambria]=${CAMBRIA_MATH:-$HOME/.local/share/fonts/CambriaMath.ttf}
  [stix]=${STIX_TWO_MATH:-$HOME/.local/share/fonts/STIXTwoMath-Regular.otf}
)

ONLY=${ONLY:-}
MATHIMG=${MATHIMG:-}
if [[ -z $MATHIMG ]]; then
  cargo build --release -p mathimg
  MATHIMG=target/release/mathimg
fi
mkdir -p docs/img
tmp=$(mktemp -d)
for eq in docs/equations/*.xml; do
  name=$(basename "$eq" .xml)
  for font in "${!FONTS[@]}"; do
    [[ -n $ONLY && $font != "$ONLY" ]] && continue
    # known layout glitch: overlapping subscript on the stacked integral in DejaVu
    [[ $name == stokes && $font == dejavu ]] && continue
    # Fira Math has no vertical or diagonal ellipsis glyphs
    [[ $name == matrix && $font == fira ]] && continue
    "$MATHIMG" -f "${FONTS[$font]}" "$eq" "$tmp/out.svg"
    sed 's/"black"/"#1f2328"/g' "$tmp/out.svg" > "docs/img/$name-$font-light.svg"
    sed 's/"black"/"#e6edf3"/g' "$tmp/out.svg" > "docs/img/$name-$font-dark.svg"
  done
done
rm -r "$tmp"
