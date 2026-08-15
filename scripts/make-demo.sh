#!/usr/bin/env bash
set -euo pipefail

# Regenerate all committed demo assets from demo-all-features.md so they can
# never go stale when the demo content or the renderer changes.
#
# Usage:
#   ./scripts/make-demo.sh [OUTPUT_DIR]     # default OUTPUT_DIR = repo root
#
# Optional overrides:
#   BIN=/path/to/markie    # binary to use (default: freshly built debug binary)
#   PNG_SCALE=2            # raster scale for the PNG demo (default: 2)
#   WIDTH=1200             # output width in pixels (default: 1200)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-$ROOT_DIR}"
BIN="${BIN:-$ROOT_DIR/target/debug/markie}"
PNG_SCALE="${PNG_SCALE:-2}"
WIDTH="${WIDTH:-1200}"
INPUT="$ROOT_DIR/demo-all-features.md"

# Default-theme demo renders (SVG + PNG + PDF).
DEFAULT_THEME="solarized_light"

# Themed variants: "output file base name|markie theme name" pairs (deterministic
# order). Each renders as both SVG and PNG, so the README gallery works in
# viewers that can't render inline SVG.
THEMED_VARIANTS=(
  "demo-dracula|dracula"
  "demo-nord|nord"
  "demo-catppuccin|catppuccin_mocha"
  "demo-solarized-dark|solarized_dark"
)

if [[ ! -f "$INPUT" ]]; then
  echo "Demo input not found: $INPUT" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"

echo "==> Demo input:    $INPUT"
echo "==> Output dir:    $OUTPUT_DIR"
echo "==> PNG scale:     $PNG_SCALE"
echo "==> Width:         $WIDTH"

if [[ "$BIN" == "$ROOT_DIR/target/debug/markie" ]]; then
  # Always rebuild the default binary so a stale build can't leak into the demos.
  echo "==> Building markie binary"
  (
    cd "$ROOT_DIR"
    cargo build
  )
elif [[ ! -x "$BIN" ]]; then
  echo "Binary not found or not executable: $BIN" >&2
  exit 1
fi

render() {
  local out="$1"
  shift
  "$BIN" "$INPUT" -o "$OUTPUT_DIR/$out" -w "$WIDTH" "$@"
}

echo "==> Rendering default theme ($DEFAULT_THEME)"
render "demo-all-features.svg" -t "$DEFAULT_THEME"
render "demo-all-features.png" --png-scale "$PNG_SCALE" -t "$DEFAULT_THEME"
render "demo-all-features.pdf" -t "$DEFAULT_THEME"

echo "==> Rendering themed variants (SVG + PNG)"
for entry in "${THEMED_VARIANTS[@]}"; do
  render "${entry%%|*}.svg" -t "${entry#*|}"
  render "${entry%%|*}.png" --png-scale "$PNG_SCALE" -t "${entry#*|}"
done

echo "==> Done. Generated files:"
{
  printf "  - %s\n" "demo-all-features.svg" "demo-all-features.png" "demo-all-features.pdf"
  for entry in "${THEMED_VARIANTS[@]}"; do
    printf "  - %s\n" "${entry%%|*}.svg" "${entry%%|*}.png"
  done
} | sort
