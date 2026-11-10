#!/usr/bin/env bash
# Builds the WebAssembly module into web/pkg and copies a default math font next to the page.
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.100
# Serve afterwards with e.g.: python3 -m http.server -d web 8000
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p mathwasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
  target/wasm32-unknown-unknown/release/mathwasm.wasm
[ -e web/font.otf ] || cp tests/testfiles/latinmodern-math.otf web/font.otf
echo "built web/pkg; serve with: python3 -m http.server -d web 8000"
