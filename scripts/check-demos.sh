#!/usr/bin/env bash
set -euo pipefail

# Verify the committed demo assets are in sync with the renderer.
#
# Usage:
#   ./scripts/check-demos.sh [GENERATED_DIR]
#
# GENERATED_DIR defaults to the repo root; typically you pass a scratch dir
# produced by `./scripts/make-demo.sh /tmp/ci-demos`. Every committed demo
# file is compared against the freshly generated one:
#
#   - SVG: compared after stripping geometry numbers (x/y/width/height/
#     points/path data/...). Those legitimately differ across platforms
#     because font metrics vary; everything else — text, colors, element
#     structure, and the root viewBox height — must match byte-for-byte.
#     This catches any renderer or demo-content change that makes the
#     committed demos stale.
#   - PNG: dimensions must match (raster bytes are platform-dependent).
#   - PDF: must exist and carry a valid header.
#
# Exits 0 when everything is in sync, 1 otherwise. Intended to run in CI so
# stale demos fail the build with a hint to run make-demo.sh.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GEN_DIR="${1:-$ROOT_DIR}"

# Demo files checked in: "name|format" pairs (deterministic order).
DEMO_FILES=(
  "demo-all-features.svg|svg"
  "demo-all-features.png|png"
  "demo-all-features.pdf|pdf"
  "demo-dracula.svg|svg"
  "demo-dracula.png|png"
  "demo-nord.svg|svg"
  "demo-nord.png|png"
  "demo-catppuccin.svg|svg"
  "demo-catppuccin.png|png"
  "demo-solarized-dark.svg|svg"
  "demo-solarized-dark.png|png"
)

fails=0

normalize_svg() {
  sed -E \
    -e 's/ (x|y|x1|y1|x2|y2|cx|cy|r|rx|ry|width|height|dy)="[0-9.eE+-]+"/ \1="#"/g' \
    -e 's/ transform="[^"]*"/ transform="#"/g' \
    -e 's/ points="[^"]*"/ points="#"/g' \
    -e 's/ d="[^"]*"/ d="#"/g' \
    "$1"
}

check_svg() {
  local name="$1"
  local gen="$GEN_DIR/$name"
  local committed="$ROOT_DIR/$name"
  local ok=1

  if [[ ! -f "$gen" ]]; then
    echo "DRIFT: $name was not generated" >&2
    return 1
  fi
  if ! diff -q <(normalize_svg "$gen") <(normalize_svg "$committed") >/dev/null; then
    echo "DRIFT: $name content differs from the committed file (text, colors, or structure changed)" >&2
    ok=0
  fi
  local gen_view committed_view
  gen_view="$(grep -o 'viewBox="[^"]*"' "$gen" || true)"
  committed_view="$(grep -o 'viewBox="[^"]*"' "$committed" || true)"
  if [[ "$gen_view" != "$committed_view" ]]; then
    echo "DRIFT: $name height changed: regenerated $gen_view vs committed $committed_view" >&2
    ok=0
  fi
  [[ "$ok" == 1 ]]
}

check_png() {
  local name="$1"
  python3 - "$GEN_DIR/$name" "$ROOT_DIR/$name" <<'PY'
import struct
import sys

def dims(path):
    with open(path, "rb") as f:
        head = f.read(33)
    if head[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    return struct.unpack(">II", head[16:24])

gen, committed = sys.argv[1], sys.argv[2]
a, b = dims(gen), dims(committed)
if a != b:
    print(f"DRIFT: {gen.split('/')[-1]} dimensions {a} != committed {b}", file=sys.stderr)
    sys.exit(1)
PY
}

check_pdf() {
  local name="$1"
  local gen="$GEN_DIR/$name"
  local committed="$ROOT_DIR/$name"
  if [[ ! -f "$gen" || ! -s "$gen" ]]; then
    echo "DRIFT: $name was not generated" >&2
    return 1
  fi
  if ! head -c 5 "$gen" | grep -q "^%PDF-"; then
    echo "DRIFT: $name is not a valid PDF" >&2
    return 1
  fi
  if [[ ! -s "$committed" ]]; then
    echo "DRIFT: committed $name is empty" >&2
    return 1
  fi
}

echo "==> Comparing committed demos against regenerated output in $GEN_DIR"
for entry in "${DEMO_FILES[@]}"; do
  name="${entry%%|*}"
  format="${entry#*|}"
  if check_$format "$name"; then
    echo "  OK   $name"
  else
    fails=$((fails + 1))
  fi
done

if [[ "$fails" -gt 0 ]]; then
  echo
  echo "==> $fails demo asset(s) are out of sync with the renderer." >&2
  echo "    Regenerate them with ./scripts/make-demo.sh and commit the results." >&2
  exit 1
fi

echo
echo "==> All demo assets are in sync."
