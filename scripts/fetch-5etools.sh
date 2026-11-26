#!/usr/bin/env bash
# Fetch the 5etools data folder (pinned release) into ./data/5etools.
# The data is not committed to this repository; run this, then the importer.
set -euo pipefail
TAG="${FIVETOOLS_TAG:-v2.41.3}"
REPO="https://github.com/5etools-mirror-3/5etools-src"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/data/5etools"

if [ -d "$DEST/.git" ]; then
  echo "Updating $DEST to $TAG"
  git -C "$DEST" fetch --depth 1 --filter=blob:none origin "refs/tags/$TAG:refs/tags/$TAG"
  git -C "$DEST" checkout -q "$TAG"
else
  mkdir -p "$ROOT/data"
  git clone --depth 1 --branch "$TAG" --filter=blob:none --sparse "$REPO" "$DEST"
  git -C "$DEST" sparse-checkout set data
fi
echo "5etools $TAG data is in $DEST/data"
echo "Next: cargo run -p importer --release -- data/5etools/data crates/web/public/compendium.json"
