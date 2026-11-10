#!/usr/bin/env bash
# Builds the WebAssembly module into web/pkg and fills in its size in the demo's header.
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.100
# Serve afterwards with e.g.: python3 -m http.server -d web 8000
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p mathwasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
  target/wasm32-unknown-unknown/release/mathwasm.wasm

# The header states the download size, so compute it from the file just built rather than keeping a
# number in the source. Replacing the element's text (not a placeholder) keeps rebuilds idempotent.
wasm=web/pkg/mathwasm_bg.wasm
gzip -9 -c "$wasm" > "$wasm.gz"
size=$(awk -v n="$(stat -c%s "$wasm")" -v g="$(stat -c%s "$wasm.gz")" \
  'BEGIN { printf "%.1f MB of WebAssembly (%d KB gzipped)", int(n / 100000) / 10, int(g / 1000) }')
rm "$wasm.gz"
sed -i -E "s|(<p class=\"moduleinfo\" id=\"moduleinfo\">)[^<]*</p>|\1$size</p>|" web/index.html

echo "built web/pkg ($size); serve with: python3 -m http.server -d web 8000"
