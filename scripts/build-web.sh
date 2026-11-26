#!/usr/bin/env bash
# Builds the web app into crates/web/dist (static files, host anywhere).
#   scripts/build-web.sh           release build
#   scripts/build-web.sh --dev     debug build (faster to compile)
# Requires: rustup target add wasm32-unknown-unknown
#           cargo install wasm-bindgen-cli --version <same as Cargo.lock>
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
PROFILE=release
FLAGS=(--release)
if [ "${1:-}" = "--dev" ]; then PROFILE=debug; FLAGS=(); fi

if [ ! -f crates/web/public/compendium.json ]; then
  echo "crates/web/public/compendium.json is missing. Run:"
  echo "  scripts/fetch-5etools.sh && cargo run -p importer --release -- data/5etools/data crates/web/public/compendium.json"
  exit 1
fi

cargo build -p web --target wasm32-unknown-unknown "${FLAGS[@]}"
OUT=crates/web/dist
rm -rf "$OUT"
mkdir -p "$OUT"
wasm-bindgen --target web --no-typescript --out-dir "$OUT/pkg" "target/wasm32-unknown-unknown/$PROFILE/web.wasm"
if command -v wasm-opt >/dev/null && [ "$PROFILE" = release ]; then
  wasm-opt -Oz -o "$OUT/pkg/web_bg.wasm" "$OUT/pkg/web_bg.wasm"
fi
cp crates/web/index.html crates/web/style.css "$OUT/"
cp -r crates/web/fonts "$OUT/fonts"
cp crates/web/public/* "$OUT/"
echo "Built $OUT — serve it with: python3 -m http.server -d $OUT 8080"
