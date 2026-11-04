#!/usr/bin/env bash
set -euo pipefail

# Run from the repository root regardless of where the script is invoked.
cd "$(dirname "$0")/.."

FONT="${FONT:-tests/testfiles/latinmodern-math.otf}"
OUT="${OUT:-svgs}"

mkdir -p "$OUT"

cargo build -p mathimg
find tests/testfiles -name '*.xml' -print0 \
    | xargs -t -P8 -I{} -0 target/debug/mathimg -f "$FONT" --show-ink-bounds --show-logical-bounds {} "$OUT"
