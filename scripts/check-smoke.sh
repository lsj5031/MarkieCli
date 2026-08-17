#!/usr/bin/env bash
set -euo pipefail

# Verify the smoke-test render output is in sync with committed baselines.
#
# Usage:
#   ./scripts/check-smoke.sh [GENERATED_DIR]
#
# GENERATED_DIR defaults to target/smoke-check; typically you pass a scratch
# dir produced for CI. The smoke fixtures in tests/smoke/ are rendered with the
# CI smoke theme and each SVG is compared against the committed baseline:
#
#   - SVG: compared after stripping geometry numbers (x/y/width/height/
#     points/path data/...). Those legitimately differ across platforms
#     because font metrics vary; everything else — text, colors, element
#     structure, and the root viewBox height — must match byte-for-byte.
#     This catches math/Mermaid rendering regressions beyond what the demo
#     drift check covers.
#
# Exits 0 when everything is in sync, 1 otherwise. Intended to run in CI so
# visual regressions fail the build with a hint to re-run make-smoke.sh.
#
# Optional overrides:
#   BIN=/path/to/markie     # binary to use (default: freshly built debug binary)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GEN_DIR="${1:-$ROOT_DIR/target/smoke-check}"
BIN="${BIN:-$ROOT_DIR/target/debug/markie}"
THEME_FILE="${THEME_FILE:-$ROOT_DIR/tests/fixtures/solarized_light.toml}"
FIXTURE_DIR="$ROOT_DIR/tests/smoke"

# Smoke fixtures checked in: "stem" of the .md input and .svg baseline.
SMOKE_FIXTURES=(
  "smoke_math"
  "smoke_mermaid"
)

normalize_svg() {
  sed -E \
    -e 's/ (x|y|x1|y1|x2|y2|cx|cy|r|rx|ry|width|height|dy)="[0-9.eE+-]+"/ \1="#"/g' \
    -e 's/ transform="[^"]*"/ transform="#"/g' \
    -e 's/ points="[^"]*"/ points="#"/g' \
    -e 's/ d="[^"]*"/ d="#"/g' \
    "$1"
}

check_svg() {
  local stem="$1"
  local gen="$GEN_DIR/$stem.svg"
  local committed="$FIXTURE_DIR/$stem.svg"
  local ok=1

  if [[ ! -f "$gen" ]]; then
    echo "DRIFT: $stem.svg was not generated" >&2
    return 1
  fi
  if ! diff -q <(normalize_svg "$gen") <(normalize_svg "$committed") >/dev/null; then
    echo "DRIFT: $stem.svg content differs from the committed baseline (text, colors, or structure changed)" >&2
    ok=0
  fi
  local gen_view committed_view
  gen_view="$(grep -o 'viewBox="[^"]*"' "$gen" || true)"
  committed_view="$(grep -o 'viewBox="[^"]*"' "$committed" || true)"
  if [[ "$gen_view" != "$committed_view" ]]; then
    echo "DRIFT: $stem.svg height changed: regenerated $gen_view vs committed $committed_view" >&2
    ok=0
  fi
  [[ "$ok" == 1 ]]
}

if [[ ! -f "$THEME_FILE" ]]; then
  echo "Theme file not found: $THEME_FILE" >&2
  exit 1
fi

if [[ "$BIN" == "$ROOT_DIR/target/debug/markie" ]]; then
  # Always rebuild the default binary so a stale build can't leak into the check.
  echo "==> Building markie binary"
  (
    cd "$ROOT_DIR"
    cargo build
  )
elif [[ ! -x "$BIN" ]]; then
  echo "Binary not found or not executable: $BIN" >&2
  exit 1
fi

mkdir -p "$GEN_DIR"

echo "==> Rendering smoke fixtures"
for stem in "${SMOKE_FIXTURES[@]}"; do
  if [[ ! -f "$FIXTURE_DIR/$stem.md" ]]; then
    echo "Smoke fixture not found: $FIXTURE_DIR/$stem.md" >&2
    exit 1
  fi
  "$BIN" "$FIXTURE_DIR/$stem.md" -o "$GEN_DIR/$stem.svg" --theme "$THEME_FILE"
done

echo "==> Comparing smoke output against committed baselines in $FIXTURE_DIR"
fails=0
for stem in "${SMOKE_FIXTURES[@]}"; do
  if check_svg "$stem"; then
    echo "  OK   $stem.svg"
  else
    fails=$((fails + 1))
  fi
done

if [[ "$fails" -gt 0 ]]; then
  echo
  echo "==> $fails smoke asset(s) are out of sync with the renderer." >&2
  echo "    Regenerate them with ./scripts/make-smoke.sh and commit the results." >&2
  exit 1
fi

echo
echo "==> All smoke assets are in sync."
