#!/usr/bin/env bash
# Builds the WebAssembly module into web/pkg and fills in its size in the demo's header.
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.100
# Optional: wasm-opt from binaryen on PATH (applied when present).
# Serve afterwards with e.g.: python3 -m http.server -d web 8000
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p formel-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
  target/wasm32-unknown-unknown/release/formel_wasm.wasm

# Shrink the module after wasm-bindgen has added its glue. Do NOT pass `--all-features`: binaryen then
# enables proposals it can encode but browsers reject, and the output fails to instantiate with
# "unknown import kind 0x7f". Enable exactly the features rustc emits for wasm32-unknown-unknown.
wasm=web/pkg/formel_wasm_bg.wasm
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz \
    --enable-mutable-globals --enable-sign-ext --enable-bulk-memory \
    --enable-nontrapping-float-to-int --enable-multivalue --enable-reference-types \
    --enable-extended-const \
    "$wasm" -o "$wasm.opt"
  mv "$wasm.opt" "$wasm"
  # A minified module no browser can instantiate is worse than a large one, so check it right here,
  # where a failure names the stage that produced it.
  if command -v node >/dev/null 2>&1; then
    node -e "new WebAssembly.Module(require('fs').readFileSync('$wasm'))" \
      || { echo "wasm-opt produced a module that does not instantiate" >&2; exit 1; }
  fi
fi

# The header states the download size, so compute it from the file just built rather than keeping a
# number in the source. Replacing the element's text (not a placeholder) keeps rebuilds idempotent.
gzip -9 -c "$wasm" > "$wasm.gz"
size=$(awk -v n="$(stat -c%s "$wasm")" -v g="$(stat -c%s "$wasm.gz")" \
  'BEGIN { printf "%.1f MB of WebAssembly (%d KB gzipped)", int(n / 100000) / 10, int(g / 1000) }')
rm "$wasm.gz"
sed -i -E "s|(<p class=\"moduleinfo\" id=\"moduleinfo\">)[^<]*</p>|\1$size</p>|" web/index.html

echo "built web/pkg ($size); serve with: python3 -m http.server -d web 8000"
