#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-/mnt/t/markie-smoke}"
BIN="$ROOT_DIR/target/debug/markie"
PNG_SCALE="${PNG_SCALE:-2.0}"

mkdir -p "$OUTPUT_DIR"

echo "==> Project root: $ROOT_DIR"
echo "==> Output dir:   $OUTPUT_DIR"
echo "==> PNG scale:    $PNG_SCALE"

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
cat >"$OUTPUT_DIR/smoke_math.md" <<'EOF'
# Math Smoke Test

Inline nth-root: $\sqrt[3]{x^3 + y^3}$

Inline binomial: $\binom{n}{k}$

Display matrix:

$$
\begin{bmatrix}
a & b \\
c & d
\end{bmatrix}
$$

Display mixed expression:

$$
\sqrt[4]{\frac{a^2 + b^2}{c^2}} + \binom{n}{2}
$$
EOF

cat >"$OUTPUT_DIR/smoke_mermaid.md" <<'EOF'
# Mermaid Smoke Test

## Flowchart

```mermaid
flowchart TD
    A[Start] --> B{Working?}
    B -->|Yes| C[Done]
    B -->|No| D[Retry]
    D --> B
```

## Sequence

```mermaid
sequenceDiagram
    participant Alice
    participant Bob
    Alice->>Bob: Hello
    Bob-->>Alice: Hi
```

## Class

```mermaid
classDiagram
    class Animal {
        +String name
        +int age
        +makeSound()
    }
    class Dog {
        +bark()
    }
    Animal <|-- Dog
```

## State

```mermaid
stateDiagram
    [*] --> Idle
    Idle --> Processing
    Processing --> [*]
```

## ER

```mermaid
erDiagram
    CUSTOMER
    ORDER
    CUSTOMER ||--o{ ORDER
```
EOF

render_all_formats() {
  local input="$1"
  local stem="$2"

  "$BIN" "$input" -o "$OUTPUT_DIR/$stem.svg"
  "$BIN" "$input" -o "$OUTPUT_DIR/$stem.png" --png-scale "$PNG_SCALE"
  "$BIN" "$input" -o "$OUTPUT_DIR/$stem.pdf"
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
