#!/usr/bin/env bash
set -euo pipefail

# Regenerate the committed smoke baselines from tests/smoke/*.md so they can
# never go stale when the smoke content or the renderer changes.
#
# Usage:
#   ./scripts/make-smoke.sh [OUTPUT_DIR]     # default OUTPUT_DIR = tests/smoke
#
# Optional overrides:
#   BIN=/path/to/markie    # binary to use (default: freshly built debug binary)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-$ROOT_DIR/tests/smoke}"
BIN="${BIN:-$ROOT_DIR/target/debug/markie}"
THEME_FILE="${THEME_FILE:-$ROOT_DIR/tests/fixtures/solarized_light.toml}"
FIXTURE_DIR="$ROOT_DIR/tests/smoke"

# Smoke fixtures: "stem" of the .md input and .svg baseline.
SMOKE_FIXTURES=(
  "smoke_math"
  "smoke_mermaid"
)

if [[ ! -f "$THEME_FILE" ]]; then
  echo "Theme file not found: $THEME_FILE" >&2
  exit 1
fi

if [[ "$BIN" == "$ROOT_DIR/target/debug/markie" ]]; then
  # Always rebuild the default binary so a stale build can't leak into the baselines.
  echo "==> Building markie binary"
  (
    cd "$ROOT_DIR"
    cargo build
  )
elif [[ ! -x "$BIN" ]]; then
  echo "Binary not found or not executable: $BIN" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"

echo "==> Fixture dir:   $FIXTURE_DIR"
echo "==> Output dir:    $OUTPUT_DIR"
echo "==> Theme file:    $THEME_FILE"

for stem in "${SMOKE_FIXTURES[@]}"; do
  if [[ ! -f "$FIXTURE_DIR/$stem.md" ]]; then
    echo "Smoke fixture not found: $FIXTURE_DIR/$stem.md" >&2
    exit 1
  fi
  echo "==> Rendering $stem"
  "$BIN" "$FIXTURE_DIR/$stem.md" -o "$OUTPUT_DIR/$stem.svg" --theme "$THEME_FILE"
done

echo "==> Done. Generated files:"
for stem in "${SMOKE_FIXTURES[@]}"; do
  printf "  - %s\n" "$stem.svg"
done
