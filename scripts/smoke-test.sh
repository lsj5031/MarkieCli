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
  shift 2

  "$BIN" "$input" -o "$OUTPUT_DIR/$stem.svg" --theme "$THEME_FILE" "$@"
  "$BIN" "$input" -o "$OUTPUT_DIR/$stem.png" --png-scale "$PNG_SCALE" --theme "$THEME_FILE" "$@"
  "$BIN" "$input" -o "$OUTPUT_DIR/$stem.pdf" --theme "$THEME_FILE" "$@"
}

echo "==> Rendering smoke outputs"
render_all_formats "$OUTPUT_DIR/smoke_math.md" "smoke_math"
render_all_formats "$OUTPUT_DIR/smoke_mermaid.md" "smoke_mermaid"

# Keep full geometry as review artifacts, separate from normalized baselines.
# Retain 640/1200 coverage for flowchart and compatibility. The document's
# three-node LR diagram needs 641px here just to reach 10px text; 800px gives
# readable headroom (~13px) rather than accepting at a font-dependent boundary.
for input in "$FIXTURE_DIR"/quality/*.md; do
  stem="$(basename "$input" .md)"
  cp "$input" "$OUTPUT_DIR/quality_$stem.md"
  widths=(640 1200)
  if [[ "$stem" == document ]]; then
    widths=(800 1200)
  fi
  for width in "${widths[@]}"; do
    render_all_formats "$input" "quality_${stem}_$width" --width "$width"
  done
done

echo "==> Checking narrow document readability rejection (retry rendered at 800px)"
for format in svg png pdf; do
  output="$OUTPUT_DIR/rejected_document_640.$format"
  if "$BIN" "$FIXTURE_DIR/quality/document.md" -o "$output" \
    --width 640 --png-scale "$PNG_SCALE" --theme "$THEME_FILE" \
    >"$output.stdout" 2>"$output.stderr"; then
    echo "Expected the 640px document to fail readability validation ($format)" >&2
    exit 1
  fi
  if [[ -e "$output" ]]; then
    echo "Rejected render has an output file: $output (use a fresh evidence directory)" >&2
    exit 1
  fi
done

echo "==> Checking unsupported syntax diagnostics"
for entry in 'nested|Nested subgraphs are not supported' 'pie|Unsupported Mermaid diagram type'; do
  stem="${entry%%|*}"
  cp "$FIXTURE_DIR/quality/unsupported/$stem.md" "$OUTPUT_DIR/unsupported_$stem.md"
  if "$BIN" "$FIXTURE_DIR/quality/unsupported/$stem.md" \
    -o "$OUTPUT_DIR/unsupported_$stem.svg" --theme "$THEME_FILE" \
    >"$OUTPUT_DIR/unsupported_$stem.stdout" 2>"$OUTPUT_DIR/unsupported_$stem.stderr"; then
    echo "Expected $stem to fail, but rendering succeeded" >&2
    exit 1
  fi
  grep -F "${entry#*|}" "$OUTPUT_DIR/unsupported_$stem.stderr"
  grep -F 'Mermaid diagram (line 3)' "$OUTPUT_DIR/unsupported_$stem.stderr"
done

# Record the actual environment; local and CI font inventories may differ.
{
  rustc --version
  printf 'PNG_SCALE=%s\nTHEME_FILE=%s\n' "$PNG_SCALE" "$THEME_FILE"
  sha256sum "$THEME_FILE" "$BIN"
  if command -v fc-list >/dev/null; then
    fc-match sans-serif
    fc-match monospace
    fc-match 'sans-serif:lang=zh'
    fc-list | sort
  fi
} >"$OUTPUT_DIR/environment.txt"

# PDF is text-as-paths: inspect a raster preview rather than relying on text
# extraction. Poppler is optional locally, installed by the evidence CI step.
if command -v pdftoppm >/dev/null; then
  for pdf in "$OUTPUT_DIR"/smoke_*.pdf "$OUTPUT_DIR"/quality_*.pdf; do
    pdftoppm -f 1 -singlefile -scale-to 1800 -png "$pdf" "${pdf%.pdf}-pdf"
  done
fi

python3 "$ROOT_DIR/scripts/check-quality.py" "$OUTPUT_DIR" "$PNG_SCALE"

echo "==> Evidence written to $OUTPUT_DIR (see docs/engine-quality.md for review limits)"
