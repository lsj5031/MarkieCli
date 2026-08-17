#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-$ROOT_DIR/target/smoke-test}"
BIN="$ROOT_DIR/target/debug/markie"
PNG_SCALE="${PNG_SCALE:-2.0}"
THEME_FILE="${THEME_FILE:-$ROOT_DIR/tests/fixtures/solarized_light.toml}"

mkdir -p "$OUTPUT_DIR"

if [[ ! -f "$THEME_FILE" ]]; then
  echo "Theme file not found: $THEME_FILE" >&2
  exit 1
fi

echo "==> Project root: $ROOT_DIR"
echo "==> Output dir:   $OUTPUT_DIR"
echo "==> PNG scale:    $PNG_SCALE"
echo "==> Theme file:   $THEME_FILE"

echo "==> Running test suite"
(
  cd "$ROOT_DIR"
  cargo test
)

echo "==> Building markie binary"
(
  cd "$ROOT_DIR"
  cargo build
)
# Smoke inputs are committed fixtures so the CI gate and the interactive
# visual check render exactly the same content.
FIXTURE_DIR="$ROOT_DIR/tests/smoke"
for stem in math mermaid; do
  if [[ ! -f "$FIXTURE_DIR/smoke_$stem.md" ]]; then
    echo "Smoke fixture not found: $FIXTURE_DIR/smoke_$stem.md" >&2
    exit 1
  fi
  cp "$FIXTURE_DIR/smoke_$stem.md" "$OUTPUT_DIR/smoke_$stem.md"
done

render_all_formats() {
  local input="$1"
  local stem="$2"

  "$BIN" "$input" -o "$OUTPUT_DIR/$stem.svg" --theme "$THEME_FILE"
  "$BIN" "$input" -o "$OUTPUT_DIR/$stem.png" --png-scale "$PNG_SCALE" --theme "$THEME_FILE"
  "$BIN" "$input" -o "$OUTPUT_DIR/$stem.pdf" --theme "$THEME_FILE"
}

echo "==> Rendering smoke outputs"
render_all_formats "$OUTPUT_DIR/smoke_math.md" "smoke_math"
render_all_formats "$OUTPUT_DIR/smoke_mermaid.md" "smoke_mermaid"

echo "==> Done. Generated files:"
printf "  - %s\n" \
  "$OUTPUT_DIR/smoke_math.md" \
  "$OUTPUT_DIR/smoke_math.svg" \
  "$OUTPUT_DIR/smoke_math.png" \
  "$OUTPUT_DIR/smoke_math.pdf" \
  "$OUTPUT_DIR/smoke_mermaid.md" \
  "$OUTPUT_DIR/smoke_mermaid.svg" \
  "$OUTPUT_DIR/smoke_mermaid.png" \
  "$OUTPUT_DIR/smoke_mermaid.pdf"

echo "==> Open for visual check (Linux):"
echo "  xdg-open $OUTPUT_DIR/smoke_math.svg"
echo "  xdg-open $OUTPUT_DIR/smoke_mermaid.svg"
echo "  xdg-open $OUTPUT_DIR/smoke_math.pdf"
echo "  xdg-open $OUTPUT_DIR/smoke_mermaid.pdf"
