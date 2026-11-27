#!/usr/bin/env bash
# Renders docs/equations/*.xml with several OpenType math fonts into docs/img/
# (set MATHIMG=path to use a prebuilt binary, ONLY=font to render a single font)
# (a light and a dark variant of each, for use with <picture> in the README).
set -euo pipefail
cd "$(dirname "$0")/.."

# Every font path can be overridden, e.g. ASANA_MATH=/path/Asana-Math.otf; a font that is not
# installed is skipped (its images are kept).
declare -A FONTS=(
  [newcm]=${NEWCM_MATH:-$HOME/.local/share/fonts/NewCMMath-Regular.otf}
  [pagella]=${PAGELLA_MATH:-/usr/share/texmf/fonts/opentype/public/tex-gyre-math/texgyrepagella-math.otf}
  [termes]=${TERMES_MATH:-/usr/share/texmf/fonts/opentype/public/tex-gyre-math/texgyretermes-math.otf}
  [asana]=${ASANA_MATH:-/usr/share/fonts/opentype/asana-math/Asana-Math.otf}
  [dejavu]=${DEJAVU_MATH:-/usr/share/fonts/truetype/dejavu/DejaVuMathTeXGyre.ttf}
  [bonum]=${BONUM_MATH:-/usr/share/texmf/fonts/opentype/public/tex-gyre-math/texgyrebonum-math.otf}
  [schola]=${SCHOLA_MATH:-/usr/share/texmf/fonts/opentype/public/tex-gyre-math/texgyreschola-math.otf}
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
    if [[ ! -f ${FONTS[$font]} ]]; then
      echo "skipping $font: ${FONTS[$font]} not found" >&2
      continue
    fi
    # stylistic sets are font specific: ss03 selects the NewCM doublestruck letters
    [[ $name == stylistic-sets && $font != newcm ]] && continue
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
