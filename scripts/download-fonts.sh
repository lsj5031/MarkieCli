#!/usr/bin/env bash
set -euo pipefail

FONT_DIR="./fonts"
mkdir -p "$FONT_DIR"

download() {
  local url="$1"
  local out="$2"

  echo "Downloading $(basename "$out")..."
  if ! curl -fL --retry 3 --retry-delay 1 -o "$out" "$url"; then
    rm -f "$out"
    echo "Failed to download $(basename "$out")" >&2
    return 1
  fi

  if [[ ! -s "$out" ]]; then
    rm -f "$out"
    echo "Downloaded empty file for $(basename "$out")" >&2
    return 1
  fi
}

download \
  "https://raw.githubusercontent.com/googlefonts/fira-sans/main/ttf/FiraSans-Regular.ttf" \
  "$FONT_DIR/FiraSans-Regular.ttf"

download \
  "https://raw.githubusercontent.com/googlefonts/fira-sans/main/ttf/FiraSans-Bold.ttf" \
  "$FONT_DIR/FiraSans-Bold.ttf"

download \
  "https://raw.githubusercontent.com/tonsky/FiraCode/master/distr/ttf/FiraCode-Regular.ttf" \
  "$FONT_DIR/FiraCode-Regular.ttf"

echo "Font download complete."
